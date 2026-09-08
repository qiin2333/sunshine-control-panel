//! Clipboard sync agent (user-session half).
//!
//! Sunshine runs as LocalSystem in Session 0 and cannot reach the interactive
//! desktop's clipboard. This Tauri-side module is the user-session agent: it
//! pulls inbound clipboard events from the service over SSE and pushes local
//! clipboard changes back to the service via HTTP.
//!
//! Wire format (v1, little-endian):
//!
//! ```text
//!   u8 version=1
//!   u8 kind        (1 = utf8 text, 2 = png image, 3 = blob ref JSON,
//!                   4 = file-transfer offer JSON)
//!   u32 token      (echo-suppression nonce)
//!   u32 length
//!   bytes payload  (length bytes)
//! ```
//!
//! kind=3 (REF) payload is a small UTF-8 JSON object:
//! `{"id":"<uuid>","mime":"image/png","size":12345}`. The actual blob is
//! transferred out-of-band over HTTPS (`/api/v1/clipboard/blob[/<id>]`) so we
//! can move payloads larger than the single-packet 65 KB wire ceiling.
//!
//! Compound clipboard (no wire-format change): a clipboard change carrying
//! BOTH text and an image is emitted as two consecutive frames sharing a
//! non-zero token, text frame first, image frame second. Peers without
//! aggregation apply the frames in order and end with the image — the exact
//! v1 outcome — so no version negotiation is needed. This agent applies each
//! frame as it arrives (zero added latency) and retains the text payload;
//! when the image lands — inline or after its REF blob fetch — the clipboard
//! is upgraded with one compound write carrying both flavors. Single-flavor
//! changes keep the legacy token=0 single frame. Burst text is always inline
//! (a REF text frame would race the image frame onto the wire and flip
//! legacy peers' final state to text); the image frame may still use REF.
//!
//! Echo suppression: every locally-applied inbound payload's hash is recorded
//! before we touch the clipboard; the watcher's resulting on_clipboard_change
//! sees the matching hash and drops the candidate, breaking the otherwise
//! infinite write→watch→post→write loop.

use std::collections::{HashMap, VecDeque};
use std::sync::{
    atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use clipboard_rs::{
    common::RustImage as _, Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, RustImageData, WatcherShutdown,
};
use log::{debug, info, warn};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::sync::Notify;

use crate::sunshine::{create_https_client, create_sse_https_client, get_sunshine_url};

const WIRE_VERSION: u8 = 1;
const KIND_TEXT: u8 = 1;
const KIND_PNG: u8 = 2;
const KIND_REF: u8 = 3;
const KIND_FILE_OFFER: u8 = 4;

const MAX_TEXT_BYTES: usize = 1 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 50 * 1024 * 1024; // matches service blob cap
const MAX_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;

/// Payload size at/above which we switch from inline (KIND_TEXT/KIND_PNG)
/// to out-of-band blob transfer (KIND_REF). Single-packet wire ceiling is
/// ~65525 bytes of payload, so 60000 leaves comfortable headroom.
const INLINE_THRESHOLD: usize = 60_000;

/// The service rejects /item bodies above 65500 bytes; a burst text frame
/// (10-byte header + payload) must stay inline, so larger text degrades the
/// change to image-only rather than racing a REF text frame.
const BURST_TEXT_MAX_INLINE: usize = 65_490;

/// How long burst text stays retained after its frame applied — purely a
/// memory bound; application is immediate on arrival (see module docs).
const BURST_RETENTION: Duration = Duration::from_secs(5);
const MAX_PENDING_BURSTS: usize = 32;

const MIME_TEXT: &str = "text/plain; charset=utf-8";
const MIME_PNG: &str = "image/png";

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const SSE_RECONNECT_BACKOFF: Duration = Duration::from_secs(3);
const ECHO_TTL: Duration = Duration::from_secs(5);

const TRANSPORT_STOPPED: u8 = 0;
const TRANSPORT_CONNECTING: u8 = 1;
const TRANSPORT_CONNECTED: u8 = 2;
const TRANSPORT_DISCONNECTED: u8 = 3;

static TRANSPORT_STATE: AtomicU8 = AtomicU8::new(TRANSPORT_STOPPED);
static LAST_CONNECTED_AT_MS: AtomicI64 = AtomicI64::new(0);
static LAST_TRANSPORT_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn create_sse_client() -> Result<reqwest::Client, String> {
    create_sse_https_client().map_err(|e| format!("创建 SSE HTTP 客户端失败: {}", e))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Png,
    Ref,
    FileOffer,
}

impl Kind {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            KIND_TEXT => Some(Kind::Text),
            KIND_PNG => Some(Kind::Png),
            KIND_REF => Some(Kind::Ref),
            KIND_FILE_OFFER => Some(Kind::FileOffer),
            _ => None,
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Kind::Text => KIND_TEXT,
            Kind::Png => KIND_PNG,
            Kind::Ref => KIND_REF,
            Kind::FileOffer => KIND_FILE_OFFER,
        }
    }
}

struct Frame {
    kind: Kind,
    token: u32,
    payload: Vec<u8>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct RefMeta {
    id: String,
    mime: String,
    size: u64,
}

fn encode_frame(f: &Frame) -> Vec<u8> {
    let mut out = Vec::with_capacity(10 + f.payload.len());
    out.push(WIRE_VERSION);
    out.push(f.kind.to_byte());
    out.extend_from_slice(&f.token.to_le_bytes());
    out.extend_from_slice(&(f.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&f.payload);
    out
}

fn decode_frame(bytes: &[u8]) -> Option<Frame> {
    if bytes.len() < 10 || bytes[0] != WIRE_VERSION {
        return None;
    }
    let kind = Kind::from_byte(bytes[1])?;
    let token = u32::from_le_bytes(bytes[2..6].try_into().ok()?);
    let len = u32::from_le_bytes(bytes[6..10].try_into().ok()?) as usize;
    if bytes.len() < 10 + len {
        return None;
    }
    Some(Frame {
        kind,
        token,
        payload: bytes[10..10 + len].to_vec(),
    })
}

#[derive(Clone, Copy)]
struct EchoEntry {
    kind: Kind,
    hash: u64,
    expires: Instant,
}

#[derive(Clone, Copy)]
struct ImageEchoEntry {
    hash: u64,
    expires: Instant,
}

#[derive(Default)]
struct EchoState {
    recent: VecDeque<EchoEntry>,
    recent_images: VecDeque<ImageEchoEntry>,
}

impl EchoState {
    fn record(&mut self, kind: Kind, payload: &[u8]) {
        if self.recent.len() >= 16 {
            self.recent.pop_front();
        }
        self.recent.push_back(EchoEntry {
            kind,
            hash: hash_payload(payload),
            expires: Instant::now() + ECHO_TTL,
        });
    }

    fn is_echo(&mut self, kind: Kind, payload: &[u8]) -> bool {
        let now = Instant::now();
        self.recent.retain(|e| e.expires > now);
        let h = hash_payload(payload);
        self.recent.iter().any(|e| e.kind == kind && e.hash == h)
    }

    fn record_image(&mut self, hash: u64) {
        if self.recent_images.len() >= 16 {
            self.recent_images.pop_front();
        }
        self.recent_images.push_back(ImageEchoEntry {
            hash,
            expires: Instant::now() + ECHO_TTL,
        });
    }

    fn is_image_echo(&mut self, hash: u64) -> bool {
        let now = Instant::now();
        self.recent_images.retain(|e| e.expires > now);
        self.recent_images.iter().any(|e| e.hash == hash)
    }
}

fn hash_payload(bytes: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash as _, Hasher as _};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Pixel identity of an image. The platform clipboard re-encodes image data
/// on every write/read round trip (Windows stores CF_DIB and macOS
/// synthesizes new flavors), so payload bytes are not a stable echo key for
/// images; pixels survive those lossless conversions.
fn hash_image_pixels(img: &RustImageData) -> Option<u64> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash as _, Hasher as _};
    let rgba = img.to_rgba8().ok()?;
    let mut h = DefaultHasher::new();
    rgba.dimensions().hash(&mut h);
    rgba.as_raw().hash(&mut h);
    Some(h.finish())
}

#[derive(Default)]
struct State {
    enabled: bool,
    sse_task: Option<JoinHandle<()>>,
    heartbeat_task: Option<JoinHandle<()>>,
    watcher_shutdown: Option<WatcherShutdown>,
    watcher_thread: Option<std::thread::JoinHandle<()>>,
    stop: Option<Arc<Notify>>,
    echo: Arc<Mutex<EchoState>>,
    next_token: u32,
}

static AGENT: once_cell::sync::Lazy<Mutex<State>> =
    once_cell::sync::Lazy::new(|| Mutex::new(State::default()));

#[derive(Serialize, Clone)]
pub struct ClipboardStatus {
    /// Local user-session agent is running (watcher + SSE pump active).
    pub agent_active: bool,
    /// Sunshine service has clipboard sync allowed (config not force-disabled).
    pub service_allowed: Option<bool>,
    pub transport_state: &'static str,
    pub last_connected_at_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

fn set_transport_state(state: u8, error: Option<String>) {
    TRANSPORT_STATE.store(state, Ordering::Release);
    *LAST_TRANSPORT_ERROR.lock().unwrap() = error;
    if state == TRANSPORT_CONNECTED {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        LAST_CONNECTED_AT_MS.store(now, Ordering::Release);
    }
}

fn transport_state_name(state: u8) -> &'static str {
    match state {
        TRANSPORT_CONNECTING => "connecting",
        TRANSPORT_CONNECTED => "connected",
        TRANSPORT_DISCONNECTED => "disconnected",
        _ => "stopped",
    }
}

// ---------- Outbound watcher ----------

struct WatcherCallbacks {
    echo: Arc<Mutex<EchoState>>,
    busy: Arc<AtomicBool>,
}

impl ClipboardHandler for WatcherCallbacks {
    fn on_clipboard_change(&mut self) {
        // Coalesce: if we're already mid-snapshot drop additional fires.
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let echo = self.echo.clone();
        let busy = self.busy.clone();
        // Do the (potentially blocking) clipboard read off the watcher thread
        // so we don't stall additional events. spawn_blocking is fine.
        tauri::async_runtime::spawn(async move {
            tauri::async_runtime::spawn_blocking(move || snapshot_and_post(&echo))
                .await
                .ok();
            busy.store(false, Ordering::Release);
        });
    }
}

fn snapshot_and_post(echo: &Arc<Mutex<EchoState>>) {
    let ctx = match ClipboardContext::new() {
        Ok(c) => c,
        Err(e) => {
            debug!("clipboard ctx open failed: {e}");
            return;
        }
    };

    // A mixed clipboard (image + fallback text label from browsers/IM, or a
    // genuinely compound copy) is emitted as a compound burst: text frame
    // first, image second, one shared non-zero token. Peers without
    // aggregation apply the frames in order and keep the image — the exact
    // v1 outcome. Single-flavor changes keep the legacy token=0 single frame.
    if let Ok(files) = ctx.get_files() {
        // Explorer file copies may include a thumbnail/icon bitmap; the
        // protocol cannot carry file references, so skip the snapshot rather
        // than sync (and later echo-replace) the fallback image. Mirrors the
        // Qt client's hasFileReferences() guard.
        if !files.is_empty() {
            debug!("local file clipboard detected; sync skipped");
            return;
        }
    }

    let text = match ctx.get_text() {
        Ok(t) if !t.is_empty() => Some(t),
        _ => None,
    };
    let img = ctx.get_image().ok().filter(|i| !i.is_empty());
    let png_bytes = img.as_ref().and_then(|i| match i.to_png() {
        Ok(p) => {
            let bytes = p.get_bytes().to_vec();
            if bytes.is_empty() {
                None
            } else {
                Some(bytes)
            }
        }
        Err(e) => {
            debug!("clipboard image to_png failed: {e}");
            None
        }
    });
    let pixel_hash = img.as_ref().and_then(hash_image_pixels);

    if let Some(pb) = &png_bytes {
        if pb.len() > MAX_IMAGE_BYTES {
            warn!(
                "local clipboard png {}B exceeds {}B cap; dropped",
                pb.len(),
                MAX_IMAGE_BYTES
            );
            return;
        }
    }

    let text_bytes = match text {
        Some(t) => {
            let bytes = t.into_bytes();
            if bytes.len() > MAX_TEXT_BYTES {
                warn!(
                    "local clipboard text {}B exceeds {}B cap; dropped",
                    bytes.len(),
                    MAX_TEXT_BYTES
                );
                return;
            }
            Some(bytes)
        }
        None => None,
    };

    match (text_bytes, png_bytes) {
        (Some(tb), Some(pb)) => {
            if tb.len() > BURST_TEXT_MAX_INLINE {
                // Burst text must stay inline (a REF text frame would race
                // the image frame onto the wire and flip legacy peers'
                // final state to text); degrade to image-only.
                info!(
                    "clipboard burst text {}B exceeds inline cap; sending image only",
                    tb.len()
                );
                if !png_payload_is_echo(echo, &pb, pixel_hash) {
                    post_outbound(Kind::Png, pb, MIME_PNG);
                }
                return;
            }

            let (text_echo, png_echo) = {
                let mut st = echo.lock().unwrap();
                let te = st.is_echo(Kind::Text, &tb);
                let pe =
                    st.is_echo(Kind::Png, &pb) || pixel_hash.map_or(false, |h| st.is_image_echo(h));
                if !te {
                    st.record(Kind::Text, &tb);
                }
                if !pe {
                    if let Some(h) = pixel_hash {
                        st.record_image(h);
                    }
                }
                (te, pe)
            };

            match (text_echo, png_echo) {
                (true, true) => {}
                (true, false) => post_outbound(Kind::Png, pb, MIME_PNG),
                (false, true) => post_outbound(Kind::Text, tb, MIME_TEXT),
                (false, false) => post_burst(tb, pb),
            }
        }
        (None, Some(pb)) => {
            if !png_payload_is_echo(echo, &pb, pixel_hash) {
                post_outbound(Kind::Png, pb, MIME_PNG);
            }
        }
        (Some(tb), None) => {
            if !echo.lock().unwrap().is_echo(Kind::Text, &tb) {
                post_outbound(Kind::Text, tb, MIME_TEXT);
            }
        }
        (None, None) => {}
    }
}

/// Echo check for an outbound image payload (byte hash + pixel hash). The
/// pixel hash is recorded when the payload passes so a delayed platform
/// re-encode of the same image is still recognized as an echo.
fn png_payload_is_echo(echo: &Arc<Mutex<EchoState>>, png: &[u8], pixel_hash: Option<u64>) -> bool {
    let mut st = echo.lock().unwrap();
    if st.is_echo(Kind::Png, png) {
        return true;
    }
    if let Some(h) = pixel_hash {
        if st.is_image_echo(h) {
            return true;
        }
        st.record_image(h);
    }
    false
}

/// Emit a compound burst: one spawned task posting the text frame and then
/// the image frame with sequential awaits — two independent HTTP POSTs have
/// no ordering guarantee, and legacy peers rely on text-first ordering to
/// end with the image.
fn post_burst(text: Vec<u8>, png: Vec<u8>) {
    let token = next_token();
    tauri::async_runtime::spawn(async move {
        let text_frame = encode_frame(&Frame {
            kind: Kind::Text,
            token,
            payload: text,
        });
        if let Err(e) = post_item(text_frame).await {
            warn!("clipboard burst text POST failed: {e}");
            return;
        }

        if png.len() <= INLINE_THRESHOLD {
            let frame = encode_frame(&Frame {
                kind: Kind::Png,
                token,
                payload: png,
            });
            if let Err(e) = post_item(frame).await {
                warn!("clipboard burst image POST failed: {e}");
            }
        } else if let Err(e) = upload_and_post_ref(png, MIME_PNG, token).await {
            warn!("clipboard burst blob transfer failed: {e}");
        }
    });
}

/// Decide inline vs out-of-band based on payload size, then dispatch.
fn post_outbound(kind: Kind, payload: Vec<u8>, mime: &'static str) {
    if payload.len() <= INLINE_THRESHOLD {
        post_inline(kind, payload);
    } else {
        post_via_blob(kind, payload, mime);
    }
}

fn post_inline(kind: Kind, payload: Vec<u8>) {
    // Single-flavor changes stay token=0: standalone frames apply
    // immediately on aggregating peers and are never coalesced.
    let body = encode_frame(&Frame {
        kind,
        token: 0,
        payload,
    });
    tauri::async_runtime::spawn(async move {
        if let Err(e) = post_item(body).await {
            warn!("clipboard /item POST failed: {e}");
        }
    });
}

fn post_via_blob(kind: Kind, payload: Vec<u8>, mime: &'static str) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = upload_and_post_ref(payload, mime, 0).await {
            warn!("clipboard blob upload failed (kind={:?}): {e}", kind);
        }
    });
}

/// Upload a payload as an out-of-band blob and post the KIND_REF descriptor
/// frame. Used by single-flavor changes (token 0) and the image frame of a
/// compound burst (the burst token, so aggregating peers can group it).
async fn upload_and_post_ref(payload: Vec<u8>, mime: &str, token: u32) -> Result<(), String> {
    let size = payload.len() as u64;
    let id = upload_blob(payload, mime).await?;
    let meta = RefMeta {
        id,
        mime: mime.to_string(),
        size,
    };
    let json = serde_json::to_vec(&meta).map_err(|e| format!("ref json encode failed: {e}"))?;
    let body = encode_frame(&Frame {
        kind: Kind::Ref,
        token,
        payload: json,
    });
    post_item(body).await
}

fn next_token() -> u32 {
    let mut st = AGENT.lock().unwrap();
    st.next_token = st.next_token.wrapping_add(1).max(1);
    st.next_token
}

async fn post_item(body: Vec<u8>) -> Result<(), String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client()?;
    let resp = client
        .post(format!(
            "{}/api/v1/clipboard/item",
            url.trim_end_matches('/')
        ))
        .header("Content-Type", "application/octet-stream")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    Ok(())
}

pub async fn post_file_offer_payload(payload: Vec<u8>) -> Result<(), String> {
    let token = next_token();
    let body = encode_frame(&Frame {
        kind: Kind::FileOffer,
        token,
        payload,
    });
    post_item(body).await
}

async fn post_capability_once() -> Result<(), String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client()?;
    let resp = client
        .post(format!(
            "{}/api/v1/clipboard/capability",
            url.trim_end_matches('/')
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    Ok(())
}

/// POST /api/v1/clipboard/blob with raw bytes + X-Clipboard-Mime header.
/// Returns the assigned blob id on success.
async fn upload_blob(bytes: Vec<u8>, mime: &str) -> Result<String, String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client()?;
    let resp = client
        .post(format!(
            "{}/api/v1/clipboard/blob",
            url.trim_end_matches('/')
        ))
        .header("Content-Type", "application/octet-stream")
        .header("X-Clipboard-Mime", mime)
        .body(bytes)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("upload status {}", status));
    }
    let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    json.get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "upload response missing id".to_string())
}

/// GET /api/v1/clipboard/blob/<id>. Returns (bytes, mime).
async fn fetch_blob(id: &str) -> Result<(Vec<u8>, String), String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client()?;
    let resp = client
        .get(format!(
            "{}/api/v1/clipboard/blob/{}",
            url.trim_end_matches('/'),
            id
        ))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("fetch status {}", resp.status()));
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok((bytes, mime))
}

// ---------- Inbound apply ----------

/// Retained text payloads of in-flight compound bursts (frames sharing a
/// non-zero token). Text applies on arrival; the image later upgrades the
/// clipboard to one compound write using the retained bytes.
static BURSTS: once_cell::sync::Lazy<Mutex<HashMap<u32, (Vec<u8>, Instant)>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

fn prune_bursts(map: &mut HashMap<u32, (Vec<u8>, Instant)>) {
    map.retain(|_, (_, ts)| ts.elapsed() < BURST_RETENTION);
}

fn retain_burst_text(token: u32, text: Vec<u8>) {
    let mut map = BURSTS.lock().unwrap();
    prune_bursts(&mut map);
    if map.len() >= MAX_PENDING_BURSTS {
        // Defensive bound against a pathological sender.
        if let Some(oldest) = map.iter().min_by_key(|(_, (_, ts))| *ts).map(|(k, _)| *k) {
            map.remove(&oldest);
        }
    }
    map.insert(token, (text, Instant::now()));
}

fn take_burst_text(token: u32) -> Option<Vec<u8>> {
    let mut map = BURSTS.lock().unwrap();
    prune_bursts(&mut map);
    map.remove(&token).map(|(text, _)| text)
}

/// Frames are applied synchronously here, in SSE stream order: the
/// compound-upgrade logic relies on text-before-image processing, which
/// per-frame spawns onto a thread pool cannot guarantee (and which, prior
/// to this, could reorder plain v1 multi-frame applies too).
fn handle_inbound_frame(frame: Frame, echo: &Arc<Mutex<EchoState>>) {
    match frame.kind {
        Kind::Ref => {
            let echo = echo.clone();
            tauri::async_runtime::spawn(async move {
                apply_inbound_ref(frame, echo).await;
            });
        }
        _ if frame.token == 0 => apply_inbound_inline(frame, echo),
        Kind::Text => {
            retain_burst_text(frame.token, frame.payload.clone());
            apply_inbound_inline(frame, echo);
        }
        Kind::Png => {
            let token = frame.token;
            deliver_burst_png(token, &frame.payload, echo);
        }
        Kind::FileOffer => {
            // Keep the legacy warn-and-drop path (apply_inbound_inline
            // rejects file offers on the host agent).
            apply_inbound_inline(frame, echo);
        }
    }
}

fn deliver_burst_png(token: u32, png: &[u8], echo: &Arc<Mutex<EchoState>>) {
    match take_burst_text(token) {
        Some(text) => apply_compound(&text, png, echo),
        None => apply_inbound_inline(
            Frame {
                kind: Kind::Png,
                token,
                payload: png.to_vec(),
            },
            echo,
        ),
    }
}

async fn apply_inbound_ref(frame: Frame, echo: Arc<Mutex<EchoState>>) {
    let meta: RefMeta = match serde_json::from_slice(&frame.payload) {
        Ok(m) => m,
        Err(e) => {
            warn!("inbound REF: bad json: {e}");
            return;
        }
    };
    if meta.id.is_empty() || meta.id.len() > 128 {
        warn!("inbound REF: bad id length");
        return;
    }
    let (bytes, _mime_from_header) = match fetch_blob(&meta.id).await {
        Ok(t) => t,
        Err(e) => {
            warn!("inbound REF: fetch_blob({}) failed: {e}", meta.id);
            return;
        }
    };
    // Trust the reference's mime (set by the original poster).
    let kind = match meta.mime.as_str() {
        m if m.starts_with("text/") => Kind::Text,
        "image/png" => Kind::Png,
        "application/vnd.sunshine.file-offer+json" => Kind::FileOffer,
        other => {
            warn!("inbound REF: unsupported mime '{}'", other);
            return;
        }
    };
    let token = frame.token;

    if token != 0 && matches!(kind, Kind::Text | Kind::Png) {
        // Blob payload of a compound burst: route through the aggregator so
        // the image can upgrade the already-applied text flavor.
        match kind {
            Kind::Text => {
                retain_burst_text(token, bytes.clone());
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    apply_inbound_inline(
                        Frame {
                            kind: Kind::Text,
                            token,
                            payload: bytes,
                        },
                        &echo,
                    )
                })
                .await;
            }
            Kind::Png => {
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    deliver_burst_png(token, &bytes, &echo)
                })
                .await;
            }
            _ => unreachable!(),
        }
        return;
    }

    let frame = Frame {
        kind,
        token,
        payload: bytes,
    };
    let echo = echo.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || apply_inbound_inline(frame, &echo)).await;
}

/// Write text + image flavors in one clipboard transaction. Records every
/// echo identity first so the watcher's snapshot of the compound write is
/// suppressed (byte hashes + pixel hash).
fn apply_compound(text: &[u8], png: &[u8], echo: &Arc<Mutex<EchoState>>) {
    // Preserve a pending local file paste (mirrors the inline-path guard).
    if let Ok(ctx) = ClipboardContext::new() {
        if let Ok(files) = ctx.get_files() {
            if !files.is_empty() {
                debug!("local file clipboard detected; inbound compound dropped");
                return;
            }
        }
    }

    let text_str = match std::str::from_utf8(text) {
        Ok(s) if !s.is_empty() && !s.contains('\0') => s,
        _ => {
            warn!("inbound compound text unusable; applying image only");
            apply_inbound_inline(
                Frame {
                    kind: Kind::Png,
                    token: 0,
                    payload: png.to_vec(),
                },
                echo,
            );
            return;
        }
    };

    // Bound decoded pixel count (mirrors the inline PNG path).
    let cursor = std::io::Cursor::new(png);
    if let Ok(reader) = image::ImageReader::new(cursor).with_guessed_format() {
        if let Ok((w, h)) = reader.into_dimensions() {
            if (w as u64) * (h as u64) > MAX_IMAGE_PIXELS {
                warn!(
                    "inbound image {}x{} exceeds {} pixel cap; dropped",
                    w, h, MAX_IMAGE_PIXELS
                );
                return;
            }
        }
    }
    let img = match RustImageData::from_bytes(png) {
        Ok(i) => i,
        Err(e) => {
            warn!("RustImageData::from_bytes failed: {e}");
            return;
        }
    };
    let pixel_hash = hash_image_pixels(&img);

    {
        let mut st = echo.lock().unwrap();
        st.record(Kind::Text, text);
        st.record(Kind::Png, png);
        if let Some(h) = pixel_hash {
            st.record_image(h);
        }
    }

    #[cfg(windows)]
    {
        match img.to_rgba8() {
            Ok(rgba) => match crate::win_clipboard::write_compound(text_str, &rgba, png) {
                Ok(()) => {
                    debug!(
                        "applied compound clipboard (text {}B + PNG {}B)",
                        text.len(),
                        png.len()
                    );
                    return;
                }
                Err(e) => {
                    warn!("compound clipboard write failed: {e}; falling back to image only");
                }
            },
            Err(e) => {
                warn!("compound rgba conversion failed: {e}; falling back to image only");
            }
        }
        if let Ok(ctx) = ClipboardContext::new() {
            let _ = ctx.set_image(img);
        }
        return;
    }

    #[cfg(not(windows))]
    {
        // clipboard-rs has no compound write off-Windows; degrade to
        // sequential writes, image last (v1 outcome).
        if let Ok(ctx) = ClipboardContext::new() {
            let _ = ctx.set_text(text_str.to_string());
            let _ = ctx.set_image(img);
        }
        debug!(
            "applied compound clipboard via sequential fallback (text {}B + PNG {}B)",
            text.len(),
            png.len()
        );
    }
}

fn apply_inbound_inline(frame: Frame, echo: &Arc<Mutex<EchoState>>) {
    // Preserve a local file clipboard: Explorer/Finder copies can carry a
    // thumbnail image, and overwriting them with remote content would
    // destroy the pending file paste. Mirrors the Qt client's guard.
    if let Ok(ctx) = ClipboardContext::new() {
        if let Ok(files) = ctx.get_files() {
            if !files.is_empty() {
                debug!("local file clipboard detected; inbound dropped");
                return;
            }
        }
    }

    // Record BEFORE writing so the watcher sees the hash and suppresses.
    echo.lock().unwrap().record(frame.kind, &frame.payload);

    let ctx = match ClipboardContext::new() {
        Ok(c) => c,
        Err(e) => {
            warn!("inbound: ClipboardContext::new failed: {e}");
            return;
        }
    };

    match frame.kind {
        Kind::Text => {
            let text = match String::from_utf8(frame.payload) {
                Ok(s) => s,
                Err(_) => {
                    warn!("inbound text not valid utf8; dropped");
                    return;
                }
            };
            if let Err(e) = ctx.set_text(text) {
                warn!("inbound set_text failed: {e}");
            }
        }
        Kind::Png => {
            // Bound decoded pixel count.
            let cursor = std::io::Cursor::new(&frame.payload);
            if let Ok(reader) = image::ImageReader::new(cursor).with_guessed_format() {
                if let Ok((w, h)) = reader.into_dimensions() {
                    if (w as u64) * (h as u64) > MAX_IMAGE_PIXELS {
                        warn!(
                            "inbound image {}x{} exceeds {} pixel cap; dropped",
                            w, h, MAX_IMAGE_PIXELS
                        );
                        return;
                    }
                }
            }
            let img = match RustImageData::from_bytes(&frame.payload) {
                Ok(i) => i,
                Err(e) => {
                    warn!("RustImageData::from_bytes failed: {e}");
                    return;
                }
            };
            // Record the pixel identity alongside the payload hash recorded
            // above: after set_image the platform re-encodes the bitmap and
            // the watcher's payload bytes no longer match, but pixels do.
            if let Some(h) = hash_image_pixels(&img) {
                echo.lock().unwrap().record_image(h);
            }
            if let Err(e) = ctx.set_image(img) {
                warn!("inbound set_image failed: {e}");
            }
        }
        Kind::Ref => {
            // Already unwrapped above; should never reach here.
            warn!("apply_inbound_inline got Kind::Ref; dropped");
        }
        Kind::FileOffer => {
            // Host GUI currently only sends this to clients. If a client sends
            // one back, ignore it rather than writing arbitrary file metadata
            // to the host clipboard.
            warn!("inbound file offer on host GUI agent; dropped");
        }
    }
}

// ---------- SSE pump ----------

async fn sse_pump(stop: Arc<Notify>, echo: Arc<Mutex<EchoState>>) {
    use futures_util::StreamExt as _;

    'outer: loop {
        // Bail-out check.
        tokio::select! {
            _ = stop.notified() => return,
            _ = tokio::time::sleep(Duration::from_millis(0)) => {}
        }
        set_transport_state(TRANSPORT_CONNECTING, None);

        let url = match get_sunshine_url().await {
            Ok(u) => u,
            Err(e) => {
                warn!("clipboard SSE: get_sunshine_url: {e}");
                set_transport_state(TRANSPORT_DISCONNECTED, Some(e));
                if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
                    return;
                }
                continue;
            }
        };
        let endpoint = format!("{}/api/v1/clipboard/events", url.trim_end_matches('/'));

        let client = match create_sse_client() {
            Ok(c) => c,
            Err(e) => {
                warn!("clipboard SSE: client: {e}");
                set_transport_state(TRANSPORT_DISCONNECTED, Some(e));
                if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
                    return;
                }
                continue;
            }
        };

        let resp = match client
            .get(&endpoint)
            .header("Accept", "text/event-stream")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("clipboard SSE connect failed: {e}");
                set_transport_state(TRANSPORT_DISCONNECTED, Some(e.to_string()));
                if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
                    return;
                }
                continue;
            }
        };
        if !resp.status().is_success() {
            let error = format!("service returned {}", resp.status());
            warn!("clipboard SSE bad status: {}", resp.status());
            set_transport_state(TRANSPORT_DISCONNECTED, Some(error));
            if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
                return;
            }
            continue;
        }
        info!("clipboard SSE connected");
        set_transport_state(TRANSPORT_CONNECTED, None);
        let mut stream = resp.bytes_stream();
        let mut buf = Vec::<u8>::new();

        loop {
            tokio::select! {
                _ = stop.notified() => return,
                chunk = stream.next() => match chunk {
                    Some(Ok(bytes)) => {
                        buf.extend_from_slice(&bytes);
                        // Parse \n\n-separated SSE events.
                        while let Some(end) = find_event_end(&buf) {
                            let raw = buf.drain(..end + 2).collect::<Vec<u8>>();
                            if let Some(frame) = parse_sse_event(&raw) {
                                handle_inbound_frame(frame, &echo);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        debug!("clipboard SSE read error: {e}");
                        set_transport_state(TRANSPORT_DISCONNECTED, Some(e.to_string()));
                        break;
                    }
                    None => {
                        debug!("clipboard SSE stream ended");
                        set_transport_state(
                            TRANSPORT_DISCONNECTED,
                            Some("event stream ended".to_string()),
                        );
                        break;
                    }
                }
            }
        }

        if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
            return;
        }
        // try reconnect
        continue 'outer;
    }
}

async fn wait_or_stop(stop: &Notify, dur: Duration) -> bool {
    tokio::select! {
        _ = stop.notified() => true,
        _ = tokio::time::sleep(dur) => false,
    }
}

fn find_event_end(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\n\n")
}

fn parse_sse_event(raw: &[u8]) -> Option<Frame> {
    let s = std::str::from_utf8(raw).ok()?;
    let mut data_b64: Option<String> = None;
    for line in s.split('\n') {
        let line = line.trim_end_matches('\r');
        if let Some(rest) = line.strip_prefix("data: ") {
            data_b64 = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_b64 = Some(rest.trim_start().to_string());
        }
    }
    let b64 = data_b64?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    decode_frame(&bytes)
}

// ---------- Heartbeat ----------

async fn heartbeat_pump(stop: Arc<Notify>) {
    loop {
        if let Err(e) = post_capability_once().await {
            debug!("clipboard heartbeat failed: {e}");
        }
        if wait_or_stop(&stop, HEARTBEAT_INTERVAL).await {
            return;
        }
    }
}

// ---------- Public API: enable / disable / status ----------

pub fn start() -> Result<(), String> {
    let mut st = AGENT.lock().unwrap();
    if st.enabled {
        return Ok(());
    }
    set_transport_state(TRANSPORT_CONNECTING, None);

    let stop = Arc::new(Notify::new());
    let echo = st.echo.clone();
    let busy = Arc::new(AtomicBool::new(false));

    // Spawn watcher thread (blocking; the crate's start_watch() is sync).
    let watcher_echo = echo.clone();
    let (shutdown_tx, watcher_handle) = {
        let mut watcher =
            ClipboardWatcherContext::new().map_err(|e| format!("ClipboardWatcherContext: {e}"))?;
        let shutdown = watcher
            .add_handler(WatcherCallbacks {
                echo: watcher_echo,
                busy,
            })
            .get_shutdown_channel();
        let handle = std::thread::Builder::new()
            .name("clipboard-watcher".into())
            .spawn(move || {
                watcher.start_watch();
            })
            .map_err(|e| format!("spawn watcher thread: {e}"))?;
        (shutdown, handle)
    };

    let sse_stop = stop.clone();
    let sse_echo = echo.clone();
    let sse_task = tauri::async_runtime::spawn(async move { sse_pump(sse_stop, sse_echo).await });

    let hb_stop = stop.clone();
    let heartbeat_task = tauri::async_runtime::spawn(async move { heartbeat_pump(hb_stop).await });

    st.enabled = true;
    st.stop = Some(stop);
    st.sse_task = Some(sse_task);
    st.heartbeat_task = Some(heartbeat_task);
    st.watcher_shutdown = Some(shutdown_tx);
    st.watcher_thread = Some(watcher_handle);
    info!("clipboard sync agent started");
    Ok(())
}

#[allow(dead_code)]
pub fn stop() {
    let (stop_tx, sse, hb, watcher_shutdown, watcher_thread) = {
        let mut st = AGENT.lock().unwrap();
        if !st.enabled {
            return;
        }
        st.enabled = false;
        (
            st.stop.take(),
            st.sse_task.take(),
            st.heartbeat_task.take(),
            st.watcher_shutdown.take(),
            st.watcher_thread.take(),
        )
    };
    if let Some(s) = &stop_tx {
        s.notify_waiters();
    }
    if let Some(s) = watcher_shutdown {
        s.stop();
    }
    if let Some(t) = watcher_thread {
        let _ = t.join();
    }
    if let Some(t) = sse {
        t.abort();
    }
    if let Some(t) = hb {
        t.abort();
    }
    set_transport_state(TRANSPORT_STOPPED, None);
    info!("clipboard sync agent stopped");
}

fn agent_active() -> bool {
    AGENT.lock().unwrap().enabled
}

async fn query_service_allowed() -> Option<bool> {
    // Service exposes the effective gate at /api/v1/clipboard/capability.
    // Keep transport errors distinct from a deliberate service-side disable.
    let url = match get_sunshine_url().await {
        Ok(u) => u,
        Err(_) => return None,
    };
    let client = match create_https_client() {
        Ok(c) => c,
        Err(_) => return None,
    };
    let endpoint = format!("{}/api/v1/clipboard/capability", url.trim_end_matches('/'));
    let resp = match client.post(&endpoint).send().await {
        Ok(r) => r,
        Err(_) => return None,
    };
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return None,
    };
    json.get("clipboard_sync").and_then(|v| v.as_bool())
}

/// Start the agent in the background at app launch. The agent is harmless
/// when the service has clipboard sync force-disabled: SSE will simply be
/// rejected and outbound posts will 4xx, so we just keep retrying quietly.
pub fn auto_start() {
    if let Err(e) = start() {
        set_transport_state(TRANSPORT_DISCONNECTED, Some(e.clone()));
        warn!("clipboard auto-start failed: {e}");
    }
}

// ---------- Tauri commands ----------

#[tauri::command]
pub async fn clipboard_sync_status() -> ClipboardStatus {
    let transport_state = TRANSPORT_STATE.load(Ordering::Acquire);
    let last_connected_at_ms = LAST_CONNECTED_AT_MS.load(Ordering::Acquire);
    ClipboardStatus {
        agent_active: agent_active(),
        service_allowed: query_service_allowed().await,
        transport_state: transport_state_name(transport_state),
        last_connected_at_ms: (last_connected_at_ms > 0).then_some(last_connected_at_ms),
        last_error: LAST_TRANSPORT_ERROR.lock().unwrap().clone(),
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    #[test]
    fn transport_state_names_cover_the_agent_lifecycle() {
        assert_eq!(transport_state_name(TRANSPORT_STOPPED), "stopped");
        assert_eq!(transport_state_name(TRANSPORT_CONNECTING), "connecting");
        assert_eq!(transport_state_name(TRANSPORT_CONNECTED), "connected");
        assert_eq!(transport_state_name(TRANSPORT_DISCONNECTED), "disconnected");
    }
}

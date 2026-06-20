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
//! Echo suppression: every locally-applied inbound payload's hash is recorded
//! before we touch the clipboard; the watcher's resulting on_clipboard_change
//! sees the matching hash and drops the candidate, breaking the otherwise
//! infinite write→watch→post→write loop.

use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use base64::Engine as _;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    RustImageData, WatcherShutdown, common::RustImage as _,
};
use log::{debug, info, warn};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::sync::Notify;

use crate::sunshine::{create_https_client, get_sunshine_url};

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

const MIME_TEXT: &str = "text/plain; charset=utf-8";
const MIME_PNG: &str = "image/png";

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
const SSE_RECONNECT_BACKOFF: Duration = Duration::from_secs(3);
const ECHO_TTL: Duration = Duration::from_secs(5);

fn create_sse_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 SSE HTTP 客户端失败: {}", e))
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

#[derive(Default)]
struct EchoState {
    recent: VecDeque<EchoEntry>,
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
}

fn hash_payload(bytes: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash as _, Hasher as _};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
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

#[derive(Serialize, Clone, Copy)]
pub struct ClipboardStatus {
    /// Local user-session agent is running (watcher + SSE pump active).
    pub agent_active: bool,
    /// Sunshine service has clipboard sync allowed (config not force-disabled).
    pub service_allowed: bool,
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

    if let Ok(text) = ctx.get_text() {
        if text.is_empty() {
            return;
        }
        let bytes = text.into_bytes();
        if bytes.len() > MAX_TEXT_BYTES {
            warn!(
                "local clipboard text {}B exceeds {}B cap; dropped",
                bytes.len(),
                MAX_TEXT_BYTES
            );
            return;
        }
        if echo.lock().unwrap().is_echo(Kind::Text, &bytes) {
            return;
        }
        post_outbound(Kind::Text, bytes, MIME_TEXT);
        return;
    }

    if let Ok(img) = ctx.get_image() {
        let png = match img.to_png() {
            Ok(p) => p,
            Err(e) => {
                debug!("clipboard image to_png failed: {e}");
                return;
            }
        };
        let bytes = png.get_bytes().to_vec();
        if bytes.len() > MAX_IMAGE_BYTES {
            warn!(
                "local clipboard png {}B exceeds {}B cap; dropped",
                bytes.len(),
                MAX_IMAGE_BYTES
            );
            return;
        }
        if echo.lock().unwrap().is_echo(Kind::Png, &bytes) {
            return;
        }
        post_outbound(Kind::Png, bytes, MIME_PNG);
    }
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
    let token = next_token();
    let body = encode_frame(&Frame {
        kind,
        token,
        payload,
    });
    tauri::async_runtime::spawn(async move {
        if let Err(e) = post_item(body).await {
            warn!("clipboard /item POST failed: {e}");
        }
    });
}

fn post_via_blob(kind: Kind, payload: Vec<u8>, mime: &'static str) {
    let size = payload.len() as u64;
    tauri::async_runtime::spawn(async move {
        let id = match upload_blob(payload, mime).await {
            Ok(id) => id,
            Err(e) => {
                warn!(
                    "clipboard blob upload failed (kind={:?}, size={}): {e}",
                    kind, size
                );
                return;
            }
        };
        let meta = RefMeta {
            id,
            mime: mime.to_string(),
            size,
        };
        let json = match serde_json::to_vec(&meta) {
            Ok(v) => v,
            Err(e) => {
                warn!("clipboard ref json encode failed: {e}");
                return;
            }
        };
        let token = next_token();
        let body = encode_frame(&Frame {
            kind: Kind::Ref,
            token,
            payload: json,
        });
        if let Err(e) = post_item(body).await {
            warn!("clipboard /item POST (ref) failed: {e}");
        }
    });
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

fn apply_inbound(frame: Frame, echo: &Arc<Mutex<EchoState>>) {
    // Non-REF path is fully synchronous; REF path is dispatched by the caller
    // onto the async runtime instead (see sse_pump).
    apply_inbound_inline(frame, echo);
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
    let frame = Frame {
        kind,
        token: frame.token,
        payload: bytes,
    };
    let echo = echo.clone();
    let _ = tauri::async_runtime::spawn_blocking(move || apply_inbound_inline(frame, &echo)).await;
}

fn apply_inbound_inline(frame: Frame, echo: &Arc<Mutex<EchoState>>) {
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

        let url = match get_sunshine_url().await {
            Ok(u) => u,
            Err(e) => {
                warn!("clipboard SSE: get_sunshine_url: {e}");
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
                if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
                    return;
                }
                continue;
            }
        };
        if !resp.status().is_success() {
            warn!("clipboard SSE bad status: {}", resp.status());
            if wait_or_stop(&stop, SSE_RECONNECT_BACKOFF).await {
                return;
            }
            continue;
        }
        info!("clipboard SSE connected");
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
                                let echo = echo.clone();
                                if frame.kind == Kind::Ref {
                                    tauri::async_runtime::spawn(async move {
                                        apply_inbound_ref(frame, echo).await;
                                    });
                                } else {
                                    tauri::async_runtime::spawn_blocking(move || apply_inbound(frame, &echo));
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        debug!("clipboard SSE read error: {e}");
                        break;
                    }
                    None => {
                        debug!("clipboard SSE stream ended");
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
    info!("clipboard sync agent stopped");
}

fn agent_active() -> bool {
    AGENT.lock().unwrap().enabled
}

async fn query_service_allowed() -> bool {
    // Service exposes the effective gate at /api/v1/clipboard/capability.
    // Treat any failure (Sunshine down, network blip) as "unknown but allowed"
    // so the indicator doesn't flicker red on every reconnect.
    let url = match get_sunshine_url().await {
        Ok(u) => u,
        Err(_) => return true,
    };
    let client = match create_https_client() {
        Ok(c) => c,
        Err(_) => return true,
    };
    let endpoint = format!("{}/api/v1/clipboard/capability", url.trim_end_matches('/'));
    let resp = match client.post(&endpoint).send().await {
        Ok(r) => r,
        Err(_) => return true,
    };
    if !resp.status().is_success() {
        return true;
    }
    let json: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return true,
    };
    json.get("clipboard_sync")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
}

/// Start the agent in the background at app launch. The agent is harmless
/// when the service has clipboard sync force-disabled: SSE will simply be
/// rejected and outbound posts will 4xx, so we just keep retrying quietly.
pub fn auto_start() {
    if let Err(e) = start() {
        warn!("clipboard auto-start failed: {e}");
    }
}

// ---------- Tauri commands ----------

#[tauri::command]
pub async fn clipboard_sync_status() -> ClipboardStatus {
    ClipboardStatus {
        agent_active: agent_active(),
        service_allowed: query_service_allowed().await,
    }
}

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
//!   u8 kind        (1 = utf8 text, 2 = png image)
//!   u32 token      (echo-suppression nonce)
//!   u32 length
//!   bytes payload  (length bytes)
//! ```
//!
//! Echo suppression: every locally-applied inbound payload's hash is recorded
//! before we touch the clipboard; the watcher's resulting on_clipboard_change
//! sees the matching hash and drops the candidate, breaking the otherwise
//! infinite write→watch→post→write loop.

use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use base64::Engine as _;
use clipboard_rs::{
    common::RustImage as _, Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, RustImageData, WatcherShutdown,
};
use log::{debug, info, warn};
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tokio::sync::Notify;

use crate::sunshine::{create_https_client, get_sunshine_url};

const WIRE_VERSION: u8 = 1;
const KIND_TEXT: u8 = 1;
const KIND_PNG: u8 = 2;

const MAX_TEXT_BYTES: usize = 1 * 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 32 * 1024 * 1024;

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
}

impl Kind {
    fn from_byte(b: u8) -> Option<Self> {
        match b {
            KIND_TEXT => Some(Kind::Text),
            KIND_PNG => Some(Kind::Png),
            _ => None,
        }
    }
    fn to_byte(self) -> u8 {
        match self {
            Kind::Text => KIND_TEXT,
            Kind::Png => KIND_PNG,
        }
    }
}

struct Frame {
    kind: Kind,
    token: u32,
    payload: Vec<u8>,
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
            warn!("local clipboard text {}B exceeds {}B cap; dropped", bytes.len(), MAX_TEXT_BYTES);
            return;
        }
        if echo.lock().unwrap().is_echo(Kind::Text, &bytes) {
            return;
        }
        post_outbound(Kind::Text, bytes);
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
            warn!("local clipboard png {}B exceeds {}B cap; dropped", bytes.len(), MAX_IMAGE_BYTES);
            return;
        }
        if echo.lock().unwrap().is_echo(Kind::Png, &bytes) {
            return;
        }
        post_outbound(Kind::Png, bytes);
    }
}

fn post_outbound(kind: Kind, payload: Vec<u8>) {
    let token = {
        let mut st = AGENT.lock().unwrap();
        st.next_token = st.next_token.wrapping_add(1).max(1);
        st.next_token
    };
    let body = encode_frame(&Frame { kind, token, payload });
    tauri::async_runtime::spawn(async move {
        if let Err(e) = post_item(body).await {
            warn!("clipboard /item POST failed: {e}");
        }
    });
}

async fn post_item(body: Vec<u8>) -> Result<(), String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client()?;
    let resp = client
        .post(format!("{}/api/v1/clipboard/item", url.trim_end_matches('/')))
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

async fn post_capability_once() -> Result<(), String> {
    let url = get_sunshine_url().await?;
    let client = create_https_client()?;
    let resp = client
        .post(format!("{}/api/v1/clipboard/capability", url.trim_end_matches('/')))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }
    Ok(())
}

// ---------- Inbound apply ----------

fn apply_inbound(frame: Frame, echo: &Arc<Mutex<EchoState>>) {
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
                                tauri::async_runtime::spawn_blocking(move || apply_inbound(frame, &echo));
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
        let mut watcher = ClipboardWatcherContext::new()
            .map_err(|e| format!("ClipboardWatcherContext: {e}"))?;
        let shutdown = watcher.add_handler(WatcherCallbacks {
            echo: watcher_echo,
            busy,
        }).get_shutdown_channel();
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
    let endpoint = format!(
        "{}/api/v1/clipboard/capability",
        url.trim_end_matches('/')
    );
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

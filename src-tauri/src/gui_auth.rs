//! 从 Sunshine 获取内存鉴权 token。
//!
//! Sunshine 侧不再对回环地址盲目放行 WebUI；捆绑 GUI 通过两条通道拿到
//! 仅存于 Sunshine 进程内存的 token：
//! 1. 环境变量 `SUNSHINE_GUI_TOKEN`（standalone 模式下 Sunshine 直接拉起 GUI 时注入）；
//! 2. 命名管道 `\\.\pipe\sunshine_gui_token`（服务模式 / GUI 独立启动 / Sunshine 重启后
//!    token 轮换时），管道服务端会校验客户端进程映像路径，本机浏览器拿不到。

use log::{debug, info};
use once_cell::sync::Lazy;
use std::sync::{Mutex, RwLock};

const ENV_TOKEN: &str = "SUNSHINE_GUI_TOKEN";
#[cfg(windows)]
const TOKEN_PIPE: &str = r"\\.\pipe\sunshine_gui_token";

static TOKEN: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(token_from_env()));
static TOKEN_FETCH_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[cfg(test)]
pub(crate) static TEST_AUTH_LOCK: Lazy<tokio::sync::Mutex<()>> =
    Lazy::new(|| tokio::sync::Mutex::new(()));
#[cfg(test)]
static TEST_PIPE_TOKEN: Lazy<Mutex<Option<Option<String>>>> = Lazy::new(|| Mutex::new(None));

fn sanitize(raw: &str) -> Option<String> {
    let token = raw.trim();
    if token.len() < 16 || token.len() > 256 || !token.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(token.to_string())
}

fn token_from_env() -> Option<String> {
    std::env::var(ENV_TOKEN).ok().as_deref().and_then(sanitize)
}

#[cfg(windows)]
fn token_from_pipe() -> Option<String> {
    use std::fs::OpenOptions;
    use std::io::Read;
    use std::os::windows::fs::OpenOptionsExt;

    const ERROR_PIPE_BUSY: i32 = 231;
    const ERROR_BROKEN_PIPE: i32 = 109;
    const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
    const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;

    for attempt in 0..3 {
        match OpenOptions::new()
            .read(true)
            .security_qos_flags(SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION)
            .open(TOKEN_PIPE)
        {
            Ok(mut pipe) => {
                let mut data = Vec::new();
                let mut buf = [0_u8; 512];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => data.extend_from_slice(&buf[..n]),
                        // 服务端发完 token 即断开连接，读到 broken pipe 等价于 EOF
                        Err(e) if e.raw_os_error() == Some(ERROR_BROKEN_PIPE) => break,
                        Err(e) => {
                            debug!("读取 GUI token 管道失败: {}", e);
                            return None;
                        }
                    }
                }
                return sanitize(&String::from_utf8_lossy(&data));
            }
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) && attempt < 2 => {
                // 管道正忙于服务其他客户端，稍后重试
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                debug!("GUI token 管道不可用: {}", e);
                return None;
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn token_from_pipe() -> Option<String> {
    None
}

#[cfg(test)]
fn test_pipe_token() -> Option<Option<String>> {
    TEST_PIPE_TOKEN
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone()
}

fn initial_token() -> Option<String> {
    #[cfg(test)]
    if let Some(result) = test_pipe_token() {
        return result;
    }

    token_from_pipe().or_else(token_from_env)
}

fn read_pipe_token() -> Option<String> {
    #[cfg(test)]
    if let Some(result) = test_pipe_token() {
        return result;
    }

    token_from_pipe()
}

fn cached() -> Option<String> {
    TOKEN.read().ok().and_then(|guard| guard.clone())
}

fn store(fresh: Option<String>, clear_on_failure: bool) -> Option<String> {
    let Ok(mut guard) = TOKEN.write() else {
        return fresh;
    };
    if let Some(token) = fresh {
        if guard.as_ref() != Some(&token) {
            info!("🔑 已更新 Sunshine GUI 鉴权 token");
        }
        *guard = Some(token.clone());
        Some(token)
    } else if clear_on_failure {
        *guard = None;
        None
    } else {
        guard.clone()
    }
}

fn acquire_current_with<F>(read_pipe: F) -> Option<String>
where
    F: FnOnce() -> Option<String>,
{
    if let Some(token) = cached() {
        return Some(token);
    }

    let _fetch_guard = TOKEN_FETCH_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(token) = cached() {
        return Some(token);
    }

    store(read_pipe(), false)
}

/// 当前 token（优先缓存；缓存为空时串行尝试获取）。可能短暂阻塞（管道 I/O）。
pub fn current() -> Option<String> {
    acquire_current_with(initial_token)
}

fn refresh_with<F>(observed_token: &str, read_pipe: F) -> Option<String>
where
    F: FnOnce() -> Option<String>,
{
    let _fetch_guard = TOKEN_FETCH_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(token) = cached()
        && token != observed_token
    {
        return Some(token);
    }
    store(read_pipe(), true)
}

/// 收到 401 后刷新 token。若其他请求已刷新为新值，则直接复用该值。
pub fn refresh(observed_token: &str) -> Option<String> {
    refresh_with(observed_token, read_pipe_token)
}

/// current() 的异步封装，避免在 async 上下文中直接做管道 I/O。
pub async fn current_async() -> Option<String> {
    tokio::task::spawn_blocking(current).await.unwrap_or(None)
}

/// refresh() 的异步封装。
pub async fn refresh_async(observed_token: String) -> Option<String> {
    tokio::task::spawn_blocking(move || refresh(&observed_token))
        .await
        .unwrap_or(None)
}

#[cfg(test)]
pub(crate) struct TestAuthState {
    previous_token: Option<String>,
    previous_pipe_token: Option<Option<String>>,
}

#[cfg(test)]
impl Drop for TestAuthState {
    fn drop(&mut self) {
        if let Ok(mut guard) = TOKEN.write() {
            *guard = self.previous_token.take();
        }
        if let Ok(mut guard) = TEST_PIPE_TOKEN.lock() {
            *guard = self.previous_pipe_token.take();
        }
    }
}

#[cfg(test)]
pub(crate) fn test_set_state(
    cached_token: Option<&str>,
    pipe_token: Option<&str>,
) -> TestAuthState {
    let previous_token = TOKEN
        .write()
        .map(|mut guard| std::mem::replace(&mut *guard, cached_token.map(str::to_string)))
        .unwrap_or(None);
    let previous_pipe_token = TEST_PIPE_TOKEN
        .lock()
        .map(|mut guard| std::mem::replace(&mut *guard, Some(pipe_token.map(str::to_string))))
        .unwrap_or(None);

    TestAuthState {
        previous_token,
        previous_pipe_token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};

    #[test]
    fn sanitize_accepts_only_bounded_alphanumeric_tokens() {
        assert_eq!(
            sanitize(" 0123456789abcdef ").as_deref(),
            Some("0123456789abcdef")
        );
        assert!(sanitize("short").is_none());
        assert!(sanitize("0123456789abcde-").is_none());
        assert!(sanitize(&"a".repeat(257)).is_none());
    }

    #[tokio::test]
    async fn explicit_test_source_does_not_fall_back_to_environment() {
        let _lock = TEST_AUTH_LOCK.lock().await;
        let _state = test_set_state(None, None);

        assert!(current().is_none());
    }

    #[tokio::test]
    async fn concurrent_first_acquisition_reads_pipe_once() {
        let _lock = TEST_AUTH_LOCK.lock().await;
        let _state = test_set_state(None, None);
        let reads = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(8));

        let threads: Vec<_> = (0..8)
            .map(|_| {
                let reads = reads.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    acquire_current_with(|| {
                        reads.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        Some("0123456789abcdef".to_string())
                    })
                })
            })
            .collect();

        for thread in threads {
            assert_eq!(thread.join().unwrap().as_deref(), Some("0123456789abcdef"));
        }
        assert_eq!(reads.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn acquisition_failure_keeps_existing_token() {
        let _lock = TEST_AUTH_LOCK.lock().await;
        let _state = test_set_state(Some("0123456789abcdef"), None);

        assert_eq!(store(None, false).as_deref(), Some("0123456789abcdef"));
        assert_eq!(cached().as_deref(), Some("0123456789abcdef"));
    }

    #[tokio::test]
    async fn stale_refresh_reuses_newer_cached_token() {
        let _lock = TEST_AUTH_LOCK.lock().await;
        let _state = test_set_state(Some("fedcba9876543210"), None);

        assert_eq!(
            refresh("0123456789abcdef").as_deref(),
            Some("fedcba9876543210")
        );
        assert_eq!(cached().as_deref(), Some("fedcba9876543210"));
    }

    #[tokio::test]
    async fn forced_refresh_failure_clears_old_token() {
        let _lock = TEST_AUTH_LOCK.lock().await;
        let _state = test_set_state(Some("0123456789abcdef"), None);

        assert!(refresh("0123456789abcdef").is_none());
        assert!(cached().is_none());
    }
}

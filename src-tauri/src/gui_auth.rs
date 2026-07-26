//! 从 Sunshine 获取内存鉴权 token。
//!
//! Sunshine 侧不再对回环地址盲目放行 WebUI；捆绑 GUI 通过两条通道拿到
//! 仅存于 Sunshine 进程内存的 token：
//! 1. 环境变量 `SUNSHINE_GUI_TOKEN`（standalone 模式下 Sunshine 直接拉起 GUI 时注入）；
//! 2. 命名管道 `\\.\pipe\sunshine_gui_token`（服务模式 / GUI 独立启动 / Sunshine 重启后
//!    token 轮换时），管道服务端会校验客户端进程映像路径，本机浏览器拿不到。

use log::{debug, info};
use once_cell::sync::Lazy;
use std::sync::RwLock;

const ENV_TOKEN: &str = "SUNSHINE_GUI_TOKEN";
#[cfg(windows)]
const TOKEN_PIPE: &str = r"\\.\pipe\sunshine_gui_token";

static TOKEN: Lazy<RwLock<Option<String>>> = Lazy::new(|| RwLock::new(token_from_env()));

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
    use std::io::Read;

    const ERROR_PIPE_BUSY: i32 = 231;
    const ERROR_BROKEN_PIPE: i32 = 109;

    for attempt in 0..3 {
        match std::fs::File::open(TOKEN_PIPE) {
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

/// 当前 token（优先缓存；缓存为空时尝试获取）。可能短暂阻塞（管道 I/O）。
pub fn current() -> Option<String> {
    if let Some(token) = TOKEN.read().ok().and_then(|guard| guard.clone()) {
        return Some(token);
    }
    refresh()
}

/// 强制重新获取（Sunshine 重启后 token 会轮换，收到 401 时调用）。
pub fn refresh() -> Option<String> {
    let fresh = token_from_pipe().or_else(token_from_env);
    if let Ok(mut guard) = TOKEN.write() {
        if fresh.is_some() && *guard != fresh {
            info!("🔑 已更新 Sunshine GUI 鉴权 token");
        }
        *guard = fresh.clone();
    }
    fresh
}

/// current() 的异步封装，避免在 async 上下文中直接做管道 I/O。
pub async fn current_async() -> Option<String> {
    tokio::task::spawn_blocking(current).await.unwrap_or(None)
}

/// refresh() 的异步封装。
pub async fn refresh_async() -> Option<String> {
    tokio::task::spawn_blocking(refresh).await.unwrap_or(None)
}

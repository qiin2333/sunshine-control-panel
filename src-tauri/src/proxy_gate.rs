//! 本地代理门禁。
//!
//! 代理会替请求注入 GUI 鉴权 token，因此绝不能让本机浏览器直接使用
//! `http://127.0.0.1:480xx` 绕过 Sunshine 的密码认证。这里通过 TCP 连接表
//! 反查对端进程 PID，只放行本进程自身（Rust 侧内部调用）及其子孙进程
//! （WebView2 的 msedgewebview2.exe 进程树）。

#[cfg(windows)]
mod imp {
    use log::debug;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// 校验结果缓存（按对端临时端口）。TTL 取短值以降低临时端口复用导致
    /// 判定错位的窗口；keep-alive 连接下命中率足够高。
    static VERDICT_CACHE: Lazy<Mutex<HashMap<u16, (bool, Instant)>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));
    const CACHE_TTL: Duration = Duration::from_secs(5);

    const AF_INET: u32 = 2;
    const NO_ERROR: u32 = 0;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const LOOPBACK_BE: u32 = u32::from_le_bytes([127, 0, 0, 1]);

    /// 在 TCP 连接表中找到「本地端口 == 对端临时端口、远端端口 == 代理端口」
    /// 的回环连接，返回持有该 socket 的进程 PID。
    fn find_owner_pid(peer_port: u16, proxy_port: u16) -> Option<u32> {
        use windows::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_CONNECTIONS,
        };

        unsafe {
            let mut size = 0_u32;
            let _ = GetExtendedTcpTable(
                None,
                &mut size,
                false,
                AF_INET,
                TCP_TABLE_OWNER_PID_CONNECTIONS,
                0,
            );

            for _ in 0..3 {
                let mut buf = vec![0_u8; size.max(16) as usize];
                let ret = GetExtendedTcpTable(
                    Some(buf.as_mut_ptr() as *mut _),
                    &mut size,
                    false,
                    AF_INET,
                    TCP_TABLE_OWNER_PID_CONNECTIONS,
                    0,
                );
                if ret == ERROR_INSUFFICIENT_BUFFER {
                    continue;
                }
                if ret != NO_ERROR {
                    debug!("GetExtendedTcpTable 失败: {}", ret);
                    return None;
                }

                let table = &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                let rows =
                    std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
                for row in rows {
                    let local_port = u16::from_be(row.dwLocalPort as u16);
                    let remote_port = u16::from_be(row.dwRemotePort as u16);
                    if local_port == peer_port
                        && remote_port == proxy_port
                        && row.dwLocalAddr == LOOPBACK_BE
                    {
                        return Some(row.dwOwningPid);
                    }
                }
                return None;
            }
            None
        }
    }

    /// pid -> ppid 快照。
    fn parent_map() -> HashMap<u32, u32> {
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        };

        let mut map = HashMap::new();
        unsafe {
            let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
                return map;
            };
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    map.insert(entry.th32ProcessID, entry.th32ParentProcessID);
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(snapshot);
        }
        map
    }

    fn is_self_or_descendant(pid: u32) -> bool {
        let me = std::process::id();
        if pid == me {
            return true;
        }
        let map = parent_map();
        let mut current = pid;
        for _ in 0..16 {
            match map.get(&current) {
                Some(&parent) if parent != 0 && parent != current => {
                    if parent == me {
                        return true;
                    }
                    current = parent;
                }
                _ => return false,
            }
        }
        false
    }

    pub fn peer_allowed(peer_port: u16, proxy_port: u16) -> bool {
        // 开发调试逃生门：允许任意本机进程访问代理（例如浏览器直接调试页面）
        if std::env::var("SUNSHINE_GUI_PROXY_ALLOW_ALL").is_ok_and(|v| v == "1") {
            return true;
        }

        let now = Instant::now();
        if let Ok(cache) = VERDICT_CACHE.lock() {
            if let Some((verdict, at)) = cache.get(&peer_port) {
                if now.duration_since(*at) < CACHE_TTL {
                    return *verdict;
                }
            }
        }

        let allowed = find_owner_pid(peer_port, proxy_port)
            .map(is_self_or_descendant)
            .unwrap_or(false);

        if let Ok(mut cache) = VERDICT_CACHE.lock() {
            if cache.len() > 256 {
                cache.retain(|_, (_, at)| now.duration_since(*at) < CACHE_TTL);
            }
            cache.insert(peer_port, (allowed, now));
        }
        allowed
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn peer_allowed(_peer_port: u16, _proxy_port: u16) -> bool {
        true
    }
}

pub use imp::peer_allowed;

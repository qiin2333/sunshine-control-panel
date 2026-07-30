//! 本地代理门禁。
//!
//! 代理会替请求注入 GUI 鉴权 token，因此绝不能让本机浏览器直接使用
//! `http://127.0.0.1:480xx` 绕过 Sunshine 的密码认证。这里通过 TCP 连接表
//! 反查对端进程 PID，只放行本进程自身（Rust 侧内部调用）及其子孙进程
//! （WebView2 的 msedgewebview2.exe 进程树）。

#[cfg(windows)]
mod imp {
    use log::debug;
    use std::collections::HashMap;
    use windows::Win32::NetworkManagement::IpHelper::MIB_TCPROW_OWNER_PID;

    const AF_INET: u32 = 2;
    const NO_ERROR: u32 = 0;
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    const LOOPBACK_BE: u32 = u32::from_le_bytes([127, 0, 0, 1]);

    fn owner_pid_from_table(
        buf: &[usize],
        byte_len: usize,
        peer_port: u16,
        proxy_port: u16,
    ) -> Option<u32> {
        let storage_len = buf.len().checked_mul(std::mem::size_of::<usize>())?;
        if byte_len < std::mem::size_of::<u32>() || byte_len > storage_len {
            return None;
        }

        let base = buf.as_ptr().cast::<u8>();
        let row_count = unsafe { std::ptr::read_unaligned(base.cast::<u32>()) } as usize;
        let rows_offset = std::mem::size_of::<u32>();
        let rows_len = row_count.checked_mul(std::mem::size_of::<MIB_TCPROW_OWNER_PID>())?;
        let rows_end = rows_offset.checked_add(rows_len)?;
        if rows_end > byte_len {
            return None;
        }

        let rows_ptr = unsafe { base.add(rows_offset) }.cast::<MIB_TCPROW_OWNER_PID>();
        if (rows_ptr as usize) % std::mem::align_of::<MIB_TCPROW_OWNER_PID>() != 0 {
            return None;
        }
        let rows = unsafe { std::slice::from_raw_parts(rows_ptr, row_count) };
        rows.iter().find_map(|row| {
            let local_port = u16::from_be(row.dwLocalPort as u16);
            let remote_port = u16::from_be(row.dwRemotePort as u16);
            (local_port == peer_port && remote_port == proxy_port && row.dwLocalAddr == LOOPBACK_BE)
                .then_some(row.dwOwningPid)
        })
    }

    /// 在 TCP 连接表中找到「本地端口 == 对端临时端口、远端端口 == 代理端口」
    /// 的回环连接，返回持有该 socket 的进程 PID。
    fn find_owner_pid(peer_port: u16, proxy_port: u16) -> Option<u32> {
        use windows::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, TCP_TABLE_OWNER_PID_CONNECTIONS,
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
                let word_count = (size.max(16) as usize).div_ceil(std::mem::size_of::<usize>());
                let mut buf = vec![0_usize; word_count];
                let ret = GetExtendedTcpTable(
                    Some(buf.as_mut_ptr().cast()),
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

                return owner_pid_from_table(&buf, size as usize, peer_port, proxy_port);
            }
            None
        }
    }

    /// pid -> ppid 快照。
    fn parent_map() -> HashMap<u32, u32> {
        use windows::Win32::System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
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

        find_owner_pid(peer_port, proxy_port)
            .map(is_self_or_descendant)
            .unwrap_or(false)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn synthetic_table(rows: &[MIB_TCPROW_OWNER_PID]) -> (Vec<usize>, usize) {
            let byte_len = std::mem::size_of::<u32>()
                + rows.len() * std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
            let word_count = byte_len.div_ceil(std::mem::size_of::<usize>());
            let mut buf = vec![0_usize; word_count];
            unsafe {
                std::ptr::write(buf.as_mut_ptr().cast::<u32>(), rows.len() as u32);
                let rows_ptr = buf
                    .as_mut_ptr()
                    .cast::<u8>()
                    .add(std::mem::size_of::<u32>())
                    .cast::<MIB_TCPROW_OWNER_PID>();
                for (index, row) in rows.iter().enumerate() {
                    std::ptr::copy_nonoverlapping(row, rows_ptr.add(index), 1);
                }
            }
            (buf, byte_len)
        }

        fn row(local: u16, remote: u16, address: u32, pid: u32) -> MIB_TCPROW_OWNER_PID {
            let mut row: MIB_TCPROW_OWNER_PID = unsafe { std::mem::zeroed() };
            row.dwLocalPort = local.to_be() as u32;
            row.dwRemotePort = remote.to_be() as u32;
            row.dwLocalAddr = address;
            row.dwOwningPid = pid;
            row
        }

        #[test]
        fn parses_matching_loopback_row() {
            let (buf, byte_len) = synthetic_table(&[
                row(1111, 48000, LOOPBACK_BE, 10),
                row(2222, 48000, LOOPBACK_BE, 20),
            ]);

            assert_eq!(owner_pid_from_table(&buf, byte_len, 2222, 48000), Some(20));
            assert_eq!(owner_pid_from_table(&buf, byte_len, 3333, 48000), None);
        }

        #[test]
        fn rejects_non_loopback_and_truncated_tables() {
            let (buf, byte_len) = synthetic_table(&[row(2222, 48000, 0, 20)]);
            assert_eq!(owner_pid_from_table(&buf, byte_len, 2222, 48000), None);
            assert_eq!(owner_pid_from_table(&buf, byte_len - 1, 2222, 48000), None);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn peer_allowed(_peer_port: u16, _proxy_port: u16) -> bool {
        true
    }
}

pub use imp::peer_allowed;

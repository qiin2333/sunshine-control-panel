//! 提权运行 .bat 脚本的通用工具，被 `vmouse` / `vigem` 等驱动管理模块共享。
//!
//! 两个驱动模块只通过 `log_prefix` 区分日志文件名。

#![cfg(target_os = "windows")]

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const ELEVATION_LAUNCH_FAILED: i32 = 1223;

/// 检查当前进程是否具有管理员权限
pub fn is_elevated() -> bool {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    matches!(output, Ok(out) if String::from_utf8_lossy(&out.stdout).trim() == "True")
}

fn append_process_output(log_path: &Path, stdout: &[u8], stderr: &[u8]) {
    let Ok(mut log) = OpenOptions::new().append(true).open(log_path) else {
        return;
    };
    if !stdout.is_empty() {
        let _ = log.write_all(stdout);
    }
    if !stderr.is_empty() {
        let _ = log.write_all(stderr);
    }
}

/// 以适当权限运行 bat 脚本：已有管理员权限则直接运行，否则提权。
///
/// bat 的输出和真实退出码都会透传。日志在请求 UAC 前创建，因此提权取消或
/// 启动失败时，错误消息中给出的日志路径也仍然存在。
pub fn run_elevated(bat_path: &Path, log_prefix: &str, extra_args: &[&str]) -> Result<(), String> {
    let stem = bat_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("run");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let operation_id = format!("{}-{timestamp}", std::process::id());
    let log_path =
        std::env::temp_dir().join(format!("sunshine-{log_prefix}-{stem}-{operation_id}.log"));
    let log_str = log_path.to_string_lossy().into_owned();
    writeln!(
        File::create(&log_path).map_err(|e| format!("创建驱动日志失败: {e}"))?,
        "Sunshine elevated task: {}",
        bat_path.display()
    )
    .map_err(|e| format!("初始化驱动日志失败: {e}"))?;

    // 把额外参数拼成 cmd-safe 的字符串：每个参数用双引号包裹
    let args_cmd: String = extra_args.iter().map(|arg| format!(" \"{arg}\"")).collect();

    if is_elevated() {
        let cmd_line = format!(
            r#""{}"{} >> "{}" 2>&1"#,
            bat_path.display(),
            args_cmd,
            log_str
        );
        let output = Command::new("cmd")
            .args(["/c", &cmd_line])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {e}，日志: {log_str}"))?;

        append_process_output(&log_path, &output.stdout, &output.stderr);
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            return Err(format!("脚本执行失败 (exit {code})，日志: {log_str}"));
        }
    } else {
        // 写一份唯一 wrapper，避免连续操作互相覆盖。
        let wrapper_path = std::env::temp_dir().join(format!(
            "sunshine-{log_prefix}-{stem}-{operation_id}-wrap.bat"
        ));
        {
            let mut wrapper = File::create(&wrapper_path)
                .map_err(|e| format!("创建 wrapper 失败: {e}，日志: {log_str}"))?;
            writeln!(wrapper, "@echo off")
                .and_then(|_| writeln!(wrapper, "chcp 65001 >nul"))
                .and_then(|_| {
                    writeln!(
                        wrapper,
                        r#"call "{}"{} >> "{}" 2>&1"#,
                        bat_path.display(),
                        args_cmd,
                        log_str
                    )
                })
                .and_then(|_| writeln!(wrapper, "exit /b %ERRORLEVEL%"))
                .map_err(|e| format!("写入 wrapper 失败: {e}，日志: {log_str}"))?;
        }

        let escaped_wrapper = wrapper_path.to_string_lossy().replace('\'', "''");
        let ps_cmd = format!(
            r#"$ErrorActionPreference = 'Stop'; try {{ $p = Start-Process cmd -ArgumentList '/c','{escaped_wrapper}' -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode }} catch {{ [Console]::Error.WriteLine($_.Exception.Message); exit {ELEVATION_LAUNCH_FAILED} }}"#
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("无法启动提权脚本: {e}，日志: {log_str}"))?;

        append_process_output(&log_path, &output.stdout, &output.stderr);
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            if code == ELEVATION_LAUNCH_FAILED && !output.stderr.is_empty() {
                return Err(format!(
                    "无法启动提权脚本（管理员授权被取消或启动失败），请重试。日志: {log_str}"
                ));
            }
            return Err(format!("脚本执行失败 (exit {code})，日志: {log_str}"));
        }
        let _ = std::fs::remove_file(&wrapper_path);
    }

    // 成功操作无需保留唯一日志；失败路径会在上方返回并保留现场。
    let _ = std::fs::remove_file(&log_path);
    Ok(())
}

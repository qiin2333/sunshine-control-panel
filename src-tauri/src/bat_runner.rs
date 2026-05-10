//! 提权运行 .bat 脚本的通用工具，被 `vmouse` / `vigem` 等驱动管理模块共享。
//!
//! 设计要点见 `run_elevated` 的注释。两个驱动模块只通过 `log_prefix` 区分
//! 日志文件名，避免日志互相覆盖。

#![cfg(target_os = "windows")]

use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 检查当前进程是否具有管理员权限
pub fn is_elevated() -> bool {
    let output = Command::new("powershell")
        .args(&[
            "-NoProfile",
            "-Command",
            "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    matches!(output, Ok(out) if String::from_utf8_lossy(&out.stdout).trim() == "True")
}

/// 以适当权限运行 bat 脚本：已有管理员权限则直接运行，否则提权。
///
/// 关键点：必须把 bat 的真实 exit code 透传出来。
/// `Start-Process -Verb RunAs -Wait` 默认**不会**让 PowerShell 进程的
/// `$LASTEXITCODE` 反映子进程退出码，必须用 `-PassThru` 拿到 Process
/// 对象再读 `.ExitCode` 并 `exit` 出去，否则 bat 即使 `exit /b 87` 上层
/// 也会以为安装成功，前端只会看到“静默成功”。
///
/// 同时把 bat 的 stdout/stderr 落到 `%TEMP%\sunshine-<prefix>-<stem>.log`，
/// 失败时 Err 字符串里带上 exit code + 日志路径，前端能直接展示给用户。
///
/// 实现细节：提权场景下 `Start-Process` 不允许 RedirectStandardOutput，
/// 而把 `cmd /c "...bat... > log 2>&1"` 用 PowerShell 单字符串拼出来，
/// 多层 cmd/PS 引号嵌套很容易解析失败（cmd 直接 exit 1 而 bat 根本没跑）。
/// 因此走更稳的方式：在 `%TEMP%` 写一份只有几行的 wrapper.bat，里面自己
/// 重定向 + exit /b %ERRORLEVEL%，然后让 PowerShell 直接 Start-Process
/// 这份 wrapper（不需要任何引号嵌套）。
pub fn run_elevated(bat_path: &Path, log_prefix: &str) -> Result<(), String> {
    let stem = bat_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("run");
    let log_path = std::env::temp_dir().join(format!("sunshine-{}-{}.log", log_prefix, stem));
    let log_str = log_path.to_string_lossy().into_owned();

    if is_elevated() {
        // 已有管理员权限，cmd 自己重定向把全部输出落盘
        let cmd_line = format!(
            r#""{bat}" > "{log}" 2>&1"#,
            bat = bat_path.display(),
            log = log_str
        );
        let output = Command::new("cmd")
            .args(&["/c", &cmd_line])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {}", e))?;

        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            return Err(format!("脚本执行失败 (exit {})，日志: {}", code, log_str));
        }
    } else {
        // 写一份 wrapper bat，避免 PS+cmd 多层引号嵌套
        let wrapper_path = std::env::temp_dir()
            .join(format!("sunshine-{}-{}-wrap.bat", log_prefix, stem));
        {
            let mut f = std::fs::File::create(&wrapper_path)
                .map_err(|e| format!("创建 wrapper 失败: {}", e))?;
            // wrapper 内容：直接调原 bat，重定向输出到日志，把真 exit code 透传
            // chcp 65001 让中文输出不乱码
            writeln!(f, "@echo off").ok();
            writeln!(f, "chcp 65001 >nul").ok();
            writeln!(f, r#"call "{}" > "{}" 2>&1"#, bat_path.display(), log_str).ok();
            writeln!(f, "exit /b %ERRORLEVEL%").ok();
        }

        let ps_cmd = format!(
            r#"$p = Start-Process cmd -ArgumentList '/c','{wrap}' -Verb RunAs -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode"#,
            wrap = wrapper_path.display()
        );

        let output = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &ps_cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("启动脚本失败: {}", e))?;

        // 不立即清理 wrapper：失败时方便人工复现
        if !output.status.success() {
            let code = output.status.code().unwrap_or(-1);
            return Err(format!("脚本执行失败 (exit {})，日志: {}", code, log_str));
        }
        // 成功才清理 wrapper
        let _ = std::fs::remove_file(&wrapper_path);
    }
    Ok(())
}

//! Launch Windows HDR Calibration. Core owns display and profile inspection.

#[cfg(target_os = "windows")]
const HDR_CALIBRATION_AUMID: &str =
    r"shell:AppsFolder\MicrosoftCorporationII.WindowsHDRCalibration_8wekyb3d8bbwe!App";

#[tauri::command]
pub fn launch_windows_hdr_calibration() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;

        let to_wide = |value: &str| {
            value
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
        };
        let operation = to_wide("open");
        let target = to_wide(HDR_CALIBRATION_AUMID);
        let result = unsafe {
            ShellExecuteW(
                Some(HWND(std::ptr::null_mut())),
                PCWSTR(operation.as_ptr()),
                PCWSTR(target.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        let code = result.0 as isize;
        if code <= 32 {
            Err(format!(
                "Windows HDR Calibration is unavailable or could not be opened (ShellExecuteW code {code})"
            ))
        } else {
            Ok(())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Windows HDR Calibration is only available on Windows".to_string())
    }
}

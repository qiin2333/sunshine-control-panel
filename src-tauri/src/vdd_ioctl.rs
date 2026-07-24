//! IOCTL transport for the ZakoVDD control channel.
//!
//! Mirrors the C++ implementation in `Sunshine/src/display_device/vdd_ioctl.cpp`
//! and the contract in `Sunshine/src/display_device/vdd_control_ioctl.h`
//! (which is itself byte-identical with the driver-side mirror at
//! `Virtual-Display-Driver/Common/Include/vdd_control_ioctl.h`).
//!
//! Commands use a UTF-16 LE, NUL-terminated buffer (e.g. `"RELOAD_DRIVER"`,
//! `"CREATEMONITOR {GUID}:[..][..]"`, `"DESTROYMONITOR"`).

#![cfg(target_os = "windows")]

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
    SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::core::{GUID, PCWSTR};

// {DA9F8C2B-7E4F-49A1-9D4E-6F2B0E1A0C4D}
// MUST stay byte-identical with `GUID_DEVINTERFACE_ZAKO_VDD_CONTROL` in
// `Sunshine/src/display_device/vdd_control_ioctl.h` (authoritative source).
const GUID_DEVINTERFACE_ZAKO_VDD_CONTROL: GUID = GUID::from_values(
    0xDA9F8C2B,
    0x7E4F,
    0x49A1,
    [0x9D, 0x4E, 0x6F, 0x2B, 0x0E, 0x1A, 0x0C, 0x4D],
);

// CTL_CODE(FILE_DEVICE_UNKNOWN=0x22, 0x800, METHOD_BUFFERED=0, FILE_WRITE_DATA=0x0002)
const IOCTL_VDD_COMMAND: u32 = 0x0022_A000;

/// Three-state outcome of an IOCTL transport attempt.
/// Mirrors `display_device::vdd_ioctl::result` in C++.
pub enum IoctlResult {
    /// IOCTL completed with `STATUS_SUCCESS`.
    Success,
    /// No registered device interface (driver too old / not installed).
    InterfaceMissing,
    /// Driver was reached but rejected the IOCTL or returned an error.
    Failed {
        message: String,
        win32_error: Option<u32>,
    },
}

/// RAII for `HDEVINFO` returned by `SetupDiGetClassDevs*`.
struct DevInfoGuard(HDEVINFO);

impl Drop for DevInfoGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = SetupDiDestroyDeviceInfoList(self.0);
            }
        }
    }
}

/// RAII for a device handle from `CreateFileW`.
struct DeviceHandle(HANDLE);

impl Drop for DeviceHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Resolve the first registered VDD control device interface path.
/// Returns `None` when no interface is registered (driver missing).
unsafe fn resolve_interface_path() -> Option<Vec<u16>> {
    unsafe {
        let dev_info_raw = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_ZAKO_VDD_CONTROL),
            PCWSTR::null(),
            None,
            DIGCF_DEVICEINTERFACE | DIGCF_PRESENT,
        )
        .ok()?;

        if dev_info_raw.is_invalid() {
            return None;
        }
        let _guard = DevInfoGuard(dev_info_raw);

        let mut iface_data = SP_DEVICE_INTERFACE_DATA {
            cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };

        if SetupDiEnumDeviceInterfaces(
            dev_info_raw,
            None,
            &GUID_DEVINTERFACE_ZAKO_VDD_CONTROL,
            0,
            &mut iface_data,
        )
        .is_err()
        {
            // ERROR_NO_MORE_ITEMS or any other code: same outcome here.
            return None;
        }

        // Probe required size.
        let mut required_size: u32 = 0;
        let _ = SetupDiGetDeviceInterfaceDetailW(
            dev_info_raw,
            &iface_data,
            None,
            0,
            Some(&mut required_size),
            None,
        );
        if required_size == 0 {
            return None;
        }

        // SP_DEVICE_INTERFACE_DETAIL_DATA_W layout:
        //   u32 cbSize
        //   wchar_t DevicePath[ANYSIZE_ARRAY]
        // ABI-required cbSize is the *declared* size: 8 on 64-bit (4 bytes
        // cbSize + 2 bytes [u16; 1] + 2 bytes padding), 6 on 32-bit.
        let mut buffer = vec![0u8; required_size as usize];
        let cb_size: u32 = if cfg!(target_pointer_width = "64") {
            8
        } else {
            6
        };
        buffer[..4].copy_from_slice(&cb_size.to_le_bytes());

        let detail_ptr = buffer.as_mut_ptr() as *mut _;
        if SetupDiGetDeviceInterfaceDetailW(
            dev_info_raw,
            &iface_data,
            Some(detail_ptr),
            required_size,
            None,
            None,
        )
        .is_err()
        {
            return None;
        }

        // Decode the trailing UTF-16 NUL-terminated DevicePath, starting
        // after the 4-byte cbSize field.
        let path_bytes = &buffer[4..required_size as usize];
        let mut wide: Vec<u16> = path_bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .take_while(|c| *c != 0)
            .collect();
        if wide.is_empty() {
            return None;
        }
        wide.push(0);
        Some(wide)
    }
}

/// Return whether the modern VDD control interface is currently registered.
/// This is a non-mutating probe used by the driver-management UI.
pub fn interface_available() -> bool {
    unsafe { resolve_interface_path().is_some() }
}

/// Send a UTF-16 command buffer to the VDD driver via IOCTL.
pub fn send_command(command: &str) -> IoctlResult {
    if command.is_empty() {
        return IoctlResult::Failed {
            message: "empty command".to_string(),
            win32_error: None,
        };
    }

    unsafe {
        let path = match resolve_interface_path() {
            Some(p) => p,
            None => return IoctlResult::InterfaceMissing,
        };

        let handle_res = CreateFileW(
            PCWSTR(path.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            Some(HANDLE::default()),
        );

        let handle = match handle_res {
            Ok(h) if !h.is_invalid() => h,
            _ => {
                // Interface enumerated but CreateFileW failed: driver is
                // present but unhappy. Surfacing as `Failed` matches the
                // C++ behaviour and prevents a duplicate retry on pipe.
                let err = GetLastError().0;
                return IoctlResult::Failed {
                    message: format!("CreateFileW failed (err={err})"),
                    win32_error: Some(err),
                };
            }
        };
        let _h_guard = DeviceHandle(handle);

        // UTF-16 LE NUL-terminated, send including the trailing L'\0'.
        let cmd_wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes_to_send = (cmd_wide.len() * std::mem::size_of::<u16>()) as u32;
        let mut bytes_returned: u32 = 0;

        let ok = DeviceIoControl(
            handle,
            IOCTL_VDD_COMMAND,
            Some(cmd_wide.as_ptr() as *const _),
            bytes_to_send,
            None,
            0,
            Some(&mut bytes_returned),
            None,
        );

        if ok.is_ok() {
            IoctlResult::Success
        } else {
            let err = GetLastError().0;
            IoctlResult::Failed {
                message: format!("DeviceIoControl(IOCTL_VDD_COMMAND) failed (err={err})"),
                win32_error: Some(err),
            }
        }
    }
}

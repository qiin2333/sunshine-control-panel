//! Static validation shared by native component installers.

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAX_PE_OFFSET: u64 = 16 * 1024 * 1024;
const MAX_PE_SECTIONS: u64 = 96;

pub(crate) fn permission_error(action: &str, error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        format!("COMPONENT-PKG-005: {action} requires administrator permission")
    } else {
        format!("COMPONENT-PKG-004: {action} failed: {error}")
    }
}

pub(crate) fn validate_named_dll(
    path: &Path,
    expected_name: &str,
    max_size: u64,
) -> Result<(), String> {
    let actual_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "RTXHDR-PKG-001: selected DLL has no valid file name".to_string())?;
    if !actual_name.eq_ignore_ascii_case(expected_name) {
        return Err(format!(
            "RTXHDR-PKG-001: expected {expected_name}, selected {actual_name}"
        ));
    }
    let metadata = fs::metadata(path)
        .map_err(|error| format!("RTXHDR-PKG-002: unable to read {expected_name}: {error}"))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > max_size {
        return Err(format!(
            "RTXHDR-PKG-002: {expected_name} has an invalid size"
        ));
    }
    validate_pe_x64(path)
        .map_err(|error| format!("RTXHDR-PKG-003: {expected_name} is not a valid x64 DLL: {error}"))
}

pub(crate) fn validate_pe_x64(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let file_size = file.metadata().map_err(|error| error.to_string())?.len();
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|error| error.to_string())?;
    if &dos[..2] != b"MZ" {
        return Err("missing MZ header".to_string());
    }
    let pe_offset = u32::from_le_bytes(dos[0x3c..0x40].try_into().unwrap()) as u64;
    if !(64..=MAX_PE_OFFSET).contains(&pe_offset) || pe_offset + 24 > file_size {
        return Err("invalid PE header offset".to_string());
    }

    file.seek(SeekFrom::Start(pe_offset))
        .map_err(|error| error.to_string())?;
    let mut coff = [0u8; 24];
    file.read_exact(&mut coff)
        .map_err(|error| error.to_string())?;
    if &coff[..4] != b"PE\0\0" {
        return Err("missing PE signature".to_string());
    }
    if u16::from_le_bytes([coff[4], coff[5]]) != 0x8664 {
        return Err("machine is not AMD64".to_string());
    }
    let section_count = u16::from_le_bytes([coff[6], coff[7]]) as u64;
    if section_count == 0 || section_count > MAX_PE_SECTIONS {
        return Err("invalid PE section count".to_string());
    }
    let optional_size = u16::from_le_bytes([coff[20], coff[21]]) as u64;
    if optional_size < 0x70 {
        return Err("missing PE optional header".to_string());
    }
    if u16::from_le_bytes([coff[22], coff[23]]) & 0x2000 == 0 {
        return Err("image is not marked as a DLL".to_string());
    }
    let section_table_end = pe_offset
        .checked_add(24)
        .and_then(|value| value.checked_add(optional_size))
        .and_then(|value| value.checked_add(section_count * 40))
        .ok_or_else(|| "PE layout overflow".to_string())?;
    if section_table_end > file_size {
        return Err("PE headers extend beyond the file".to_string());
    }

    let mut optional = vec![0u8; optional_size as usize];
    file.read_exact(&mut optional)
        .map_err(|error| error.to_string())?;
    if u16::from_le_bytes([optional[0], optional[1]]) != 0x20b {
        return Err("optional header is not PE32+".to_string());
    }
    let size_of_image = u32::from_le_bytes(optional[56..60].try_into().unwrap()) as u64;
    let size_of_headers = u32::from_le_bytes(optional[60..64].try_into().unwrap()) as u64;
    if size_of_headers == 0 || size_of_headers > file_size || size_of_image < size_of_headers {
        return Err("invalid PE image or header size".to_string());
    }

    let mut has_nonempty_section = false;
    for _ in 0..section_count {
        let mut section = [0u8; 40];
        file.read_exact(&mut section)
            .map_err(|error| error.to_string())?;
        let virtual_size = u32::from_le_bytes(section[8..12].try_into().unwrap()) as u64;
        let virtual_address = u32::from_le_bytes(section[12..16].try_into().unwrap()) as u64;
        let raw_size = u32::from_le_bytes(section[16..20].try_into().unwrap()) as u64;
        let raw_offset = u32::from_le_bytes(section[20..24].try_into().unwrap()) as u64;
        if virtual_size == 0 && raw_size == 0 {
            continue;
        }
        has_nonempty_section = true;
        if raw_size > 0
            && raw_offset
                .checked_add(raw_size)
                .is_none_or(|end| raw_offset < size_of_headers || end > file_size)
        {
            return Err("PE section raw data is outside the file".to_string());
        }
        if virtual_address
            .checked_add(virtual_size.max(raw_size))
            .is_none_or(|end| end > size_of_image)
        {
            return Err("PE section virtual range is outside the image".to_string());
        }
    }
    if !has_nonempty_section {
        return Err("PE image has no nonempty sections".to_string());
    }
    Ok(())
}

pub(crate) fn sha256_file(path: &Path, maximum: u64) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut consumed = 0u64;
    loop {
        let read = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        consumed += read as u64;
        if consumed > maximum {
            return Err("component exceeds its size limit".to_string());
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

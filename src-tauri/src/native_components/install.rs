//! Static validation shared by native component installers.

use object::endian::LittleEndian as LE;
use object::pe;
use object::read::{
    ReadCache,
    pe::{ImageNtHeaders, ImageOptionalHeader},
};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
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
    let file = File::open(path).map_err(|error| error.to_string())?;
    let file_size = file.metadata().map_err(|error| error.to_string())?.len();
    // Read only the headers and section table; do not load the DLL or its symbols.
    let data = ReadCache::new(file);
    let dos = pe::ImageDosHeader::parse(&data).map_err(|error| error.to_string())?;
    let mut offset = u64::from(dos.nt_headers_offset());
    if !(64..=MAX_PE_OFFSET).contains(&offset) {
        return Err("invalid PE header offset".to_string());
    }
    let (headers, _) =
        pe::ImageNtHeaders64::parse(&data, &mut offset).map_err(|error| error.to_string())?;
    let coff = headers.file_header();
    if coff.machine.get(LE) != pe::IMAGE_FILE_MACHINE_AMD64 {
        return Err("machine is not AMD64".to_string());
    }
    let section_count = u64::from(coff.number_of_sections.get(LE));
    if section_count == 0 || section_count > MAX_PE_SECTIONS {
        return Err("invalid PE section count".to_string());
    }
    if coff.characteristics.get(LE) & pe::IMAGE_FILE_DLL == pe::FileFlags(0) {
        return Err("image is not marked as a DLL".to_string());
    }
    let optional = headers.optional_header();
    let size_of_image = u64::from(optional.size_of_image());
    let size_of_headers = u64::from(optional.size_of_headers());
    if size_of_headers == 0 || size_of_headers > file_size || size_of_image < size_of_headers {
        return Err("invalid PE image or header size".to_string());
    }
    let sections = headers
        .sections(&data, offset)
        .map_err(|error| error.to_string())?;
    let mut has_nonempty_section = false;
    for section in sections.iter() {
        let virtual_size = u64::from(section.virtual_size.get(LE));
        let virtual_address = u64::from(section.virtual_address.get(LE));
        let raw_size = u64::from(section.size_of_raw_data.get(LE));
        let raw_offset = u64::from(section.pointer_to_raw_data.get(LE));
        if virtual_size == 0 && raw_size == 0 {
            continue;
        }
        has_nonempty_section = true;
        if raw_size > 0 && (raw_offset < size_of_headers || raw_offset + raw_size > file_size) {
            return Err("PE section raw data is outside the file".to_string());
        }
        // These values originate as u32, so their sums cannot overflow u64.
        if virtual_address + virtual_size.max(raw_size) > size_of_image {
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

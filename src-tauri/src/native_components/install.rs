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
    let file = File::open(path).map_err(|error| error.to_string())?;
    if file.metadata().map_err(|error| error.to_string())?.len() > maximum {
        return Err("component exceeds its size limit".to_string());
    }
    // Keep the read bounded even if another process grows the opened file.
    let mut file = file.take(maximum.saturating_add(1));
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn file_hash_respects_size_limit() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"abc").unwrap();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert!(sha256_file(file.path(), 2).is_err());
        assert_eq!(sha256_file(file.path(), 3).unwrap(), expected);
        assert_eq!(sha256_file(file.path(), u64::MAX).unwrap(), expected);
    }

    pub(crate) fn write_test_pe(path: &Path, machine: u16, dll: bool, magic: u16, sections: u16) {
        const PE_OFFSET: usize = 0x80;
        const OPTIONAL_SIZE: usize = 0xf0;
        const RAW_OFFSET: usize = 0x400;
        const RAW_SIZE: usize = 0x200;
        let size = (PE_OFFSET + 24 + OPTIONAL_SIZE + usize::from(sections) * 40)
            .max(RAW_OFFSET + RAW_SIZE);
        let mut bytes = vec![0u8; size];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&(PE_OFFSET as u32).to_le_bytes());
        bytes[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");
        bytes[PE_OFFSET + 4..PE_OFFSET + 6].copy_from_slice(&machine.to_le_bytes());
        bytes[PE_OFFSET + 6..PE_OFFSET + 8].copy_from_slice(&sections.to_le_bytes());
        bytes[PE_OFFSET + 20..PE_OFFSET + 22]
            .copy_from_slice(&(OPTIONAL_SIZE as u16).to_le_bytes());
        let characteristics: u16 = if dll { 0x2000 } else { 0 };
        bytes[PE_OFFSET + 22..PE_OFFSET + 24].copy_from_slice(&characteristics.to_le_bytes());
        bytes[PE_OFFSET + 24..PE_OFFSET + 26].copy_from_slice(&magic.to_le_bytes());
        bytes[PE_OFFSET + 24 + 56..PE_OFFSET + 24 + 60].copy_from_slice(&(0x2000u32).to_le_bytes());
        bytes[PE_OFFSET + 24 + 60..PE_OFFSET + 24 + 64]
            .copy_from_slice(&(RAW_OFFSET as u32).to_le_bytes());
        if sections > 0 {
            let section = PE_OFFSET + 24 + OPTIONAL_SIZE;
            bytes[section + 8..section + 12].copy_from_slice(&(RAW_SIZE as u32).to_le_bytes());
            bytes[section + 12..section + 16].copy_from_slice(&(0x1000u32).to_le_bytes());
            bytes[section + 16..section + 20].copy_from_slice(&(RAW_SIZE as u32).to_le_bytes());
            bytes[section + 20..section + 24].copy_from_slice(&(RAW_OFFSET as u32).to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn accepts_x64_pe32_plus_dll() {
        let root = std::env::temp_dir().join(format!("rtx-hdr-pe-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("component.dll");
        write_test_pe(&path, 0x8664, true, 0x20b, 1);
        assert!(validate_named_dll(&path, "component.dll", u64::MAX).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_wrong_machine_kind_or_truncated_headers() {
        let root = std::env::temp_dir().join(format!("rtx-hdr-bad-pe-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        for (name, machine, dll, magic, sections) in [
            ("x86", 0x014c, true, 0x10b, 1),
            ("exe", 0x8664, false, 0x20b, 1),
            ("pe32", 0x8664, true, 0x10b, 1),
            ("nosections", 0x8664, true, 0x20b, 0),
        ] {
            let path = root.join(format!("{name}.dll"));
            write_test_pe(&path, machine, dll, magic, sections);
            assert!(validate_pe_x64(&path).is_err(), "{name} should be rejected");
        }
        let truncated = root.join("truncated.dll");
        fs::write(&truncated, b"MZ").unwrap();
        assert!(validate_pe_x64(&truncated).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_sections_outside_file_or_image_and_invalid_directory_count() {
        let root = std::env::temp_dir().join(format!("hdr-pe-bounds-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("component.dll");
        const OPTIONAL: usize = 0x80 + 24;
        const SECTION: usize = OPTIONAL + 0xf0;
        for (field, value) in [
            (SECTION + 20, 0x200u32), // raw data overlaps the headers
            (SECTION + 20, 0x800),    // raw data begins beyond EOF
            (SECTION + 16, u32::MAX), // raw data extends beyond EOF
            (SECTION + 12, 0x2000),   // virtual data begins beyond SizeOfImage
            (SECTION + 8, u32::MAX),  // virtual extent cannot wrap into the image
            (OPTIONAL + 108, 17),     // directory table exceeds the optional header
            (0x3c, u32::MAX),         // NT header offset is outside the allowed range
        ] {
            write_test_pe(&path, 0x8664, true, 0x20b, 1);
            let mut bytes = fs::read(&path).unwrap();
            bytes[field..field + 4].copy_from_slice(&value.to_le_bytes());
            fs::write(&path, bytes).unwrap();
            assert!(
                validate_pe_x64(&path).is_err(),
                "field {field:#x}, value {value:#x}"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}

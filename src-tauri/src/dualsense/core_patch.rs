//! Apply the main package's compatible Core fix to staged optional components.

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::HIDMAESTRO_VERSION;
use super::packages::sha256_file;

pub(crate) const CORE_PATCH_ID: &str = "registry-isolation-1";
pub(crate) const ORIGINAL_CORE_SHA256: &str =
    "95c6d3ec0d41cd7b3adda80da3ffbcd19c9466cc50e08a973c7745a16c7e0bde";

#[derive(Deserialize)]
pub(crate) struct CorePatchManifest {
    pub(crate) schema: u32,
    pub(crate) upstream_version: String,
    pub(crate) patch: String,
    pub(crate) original_sha256: String,
    pub(crate) sha256: String,
}

pub(crate) fn core_patch_dir() -> PathBuf {
    PathBuf::from(crate::sunshine::get_sunshine_install_path())
        .join("tools")
        .join("hidmaestro-core-patch")
}

pub(crate) fn read_core_patch(directory: &Path) -> Result<CorePatchManifest, String> {
    let manifest: CorePatchManifest = serde_json::from_slice(
        &fs::read(directory.join("core-patch.json"))
            .map_err(|error| format!("DS5-PKG-002: bundled Core fix is missing: {error}"))?,
    )
    .map_err(|error| format!("DS5-PKG-002: invalid Core fix metadata: {error}"))?;
    if manifest.schema != 1
        || manifest.upstream_version != HIDMAESTRO_VERSION
        || manifest.patch != CORE_PATCH_ID
        || manifest.original_sha256 != ORIGINAL_CORE_SHA256
        || manifest.sha256.len() != 64
        || !manifest.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("DS5-PKG-002: bundled Core fix does not match Sunshine".to_string());
    }
    let actual = sha256_file(&directory.join("HIDMaestro.Core.dll"))?;
    if !actual.eq_ignore_ascii_case(&manifest.sha256) {
        return Err("DS5-PKG-001: bundled Core fix integrity check failed".to_string());
    }
    Ok(manifest)
}

// The destination must be idle: staging, or an active component whose session
// has already been excluded by the elevated self-test's lifecycle guard.
pub(crate) fn apply_staged_core_patch(
    patch_directory: &Path,
    staging: &Path,
) -> Result<String, String> {
    let manifest = read_core_patch(patch_directory)?;
    activate_verified_core_patch(patch_directory, staging, &manifest)
}

fn activate_verified_core_patch(
    patch_directory: &Path,
    destination: &Path,
    manifest: &CorePatchManifest,
) -> Result<String, String> {
    let core = destination.join("HIDMaestro.Core.dll");
    let actual = sha256_file(&core)?;
    if actual.eq_ignore_ascii_case(&manifest.sha256) {
        return Ok(manifest.sha256.clone());
    }
    if actual != manifest.original_sha256 {
        return Err("DS5-PKG-001: Core fix only supports the pinned upstream assembly".to_string());
    }
    let temporary = destination.join(format!("HIDMaestro.Core.{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        fs::copy(patch_directory.join("HIDMaestro.Core.dll"), &temporary)
            .map_err(|error| format!("DS5-PKG-002: unable to stage Core fix: {error}"))?;
        if !sha256_file(&temporary)?.eq_ignore_ascii_case(&manifest.sha256) {
            return Err("DS5-PKG-001: staged Core fix integrity check failed".to_string());
        }
        if sha256_file(&core)? != actual {
            return Err("DS5-PKG-001: Core changed during patch activation".to_string());
        }
        fs::rename(&temporary, &core)
            .map_err(|error| format!("DS5-PKG-002: unable to activate Core fix: {error}"))?;
        Ok(manifest.sha256.clone())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PathBuf, PathBuf, PathBuf, CorePatchManifest) {
        let root =
            std::env::temp_dir().join(format!("sunshine-core-patch-{}", uuid::Uuid::new_v4()));
        let patch = root.join("patch");
        let active = root.join("active");
        fs::create_dir_all(&patch).unwrap();
        fs::create_dir_all(&active).unwrap();
        fs::write(patch.join("HIDMaestro.Core.dll"), b"patched").unwrap();
        fs::write(active.join("HIDMaestro.Core.dll"), b"original").unwrap();
        let metadata = CorePatchManifest {
            schema: 1,
            upstream_version: HIDMAESTRO_VERSION.into(),
            patch: CORE_PATCH_ID.into(),
            original_sha256: sha256_file(&active.join("HIDMaestro.Core.dll")).unwrap(),
            sha256: sha256_file(&patch.join("HIDMaestro.Core.dll")).unwrap(),
        };
        (root, patch, active, metadata)
    }

    #[test]
    fn compatible_core_is_upgraded_and_activation_is_idempotent() {
        let (root, patch, active, metadata) = fixture();
        activate_verified_core_patch(&patch, &active, &metadata).unwrap();
        assert_eq!(
            fs::read(active.join("HIDMaestro.Core.dll")).unwrap(),
            b"patched"
        );
        activate_verified_core_patch(&patch, &active, &metadata).unwrap();
        assert_eq!(fs::read_dir(&active).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_patch_and_unknown_core_preserve_the_existing_assembly() {
        let (root, patch, active, metadata) = fixture();
        fs::write(patch.join("HIDMaestro.Core.dll"), b"corrupt").unwrap();
        assert!(activate_verified_core_patch(&patch, &active, &metadata).is_err());
        assert_eq!(
            fs::read(active.join("HIDMaestro.Core.dll")).unwrap(),
            b"original"
        );
        assert_eq!(fs::read_dir(&active).unwrap().count(), 1);
        fs::write(active.join("HIDMaestro.Core.dll"), b"unknown").unwrap();
        assert!(activate_verified_core_patch(&patch, &active, &metadata).is_err());
        assert_eq!(
            fs::read(active.join("HIDMaestro.Core.dll")).unwrap(),
            b"unknown"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mismatched_upstream_patch_metadata_is_rejected() {
        let (root, patch, _, metadata) = fixture();
        fs::write(
            patch.join("core-patch.json"),
            serde_json::json!({
                "schema": metadata.schema,
                "upstream_version": metadata.upstream_version,
                "patch": metadata.patch,
                "original_sha256": metadata.original_sha256,
                "sha256": metadata.sha256,
            })
            .to_string(),
        )
        .unwrap();
        assert!(read_core_patch(&patch).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

//! Component manifests, install paths, verified downloads, and package hashing.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::github_download::{
    self, DEFAULT_IDLE_TIMEOUT, DEFAULT_RESPONSE_TIMEOUT, DownloadAttemptPhase, DownloadRequest,
};

use super::{
    COMPONENT_DOWNLOAD_OVERALL_TIMEOUT, COMPONENT_VERSION, HIDMAESTRO_SHA256, HIDMAESTRO_VERSION,
    MAX_SIDECAR_PACKAGE_BYTES, MAX_SIDECAR_PACKAGE_MANIFEST_BYTES, PROTOCOL_VERSION,
    ProgressReporter, SIDECAR_EXE, SIDECAR_PACKAGE_ASSET, SIDECAR_PACKAGE_LICENSE,
    SIDECAR_PACKAGE_MANIFEST, SIDECAR_PACKAGE_TARGET, USBIP_SHA256, report_progress,
};

#[derive(Clone, Default)]
pub(crate) struct LocalComponentPackages {
    pub(crate) sidecar: Option<PathBuf>,
    pub(crate) hidmaestro: Option<PathBuf>,
    pub(crate) usbip: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalComponentKind {
    Sidecar,
    Hidmaestro,
    Usbip,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(crate) struct InstalledComponentManifest {
    pub(crate) component_version: String,
    pub(crate) hidmaestro_version: String,
    pub(crate) sha256: String,
    pub(crate) protocol: u32,
    pub(crate) sidecar_file: String,
    pub(crate) sidecar_sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct SidecarPackageManifest {
    pub(crate) schema: u32,
    pub(crate) component_version: String,
    pub(crate) protocol: u32,
    pub(crate) target: String,
    pub(crate) license: String,
    pub(crate) asset_name: String,
    pub(crate) download_url: String,
    pub(crate) sha256: String,
    pub(crate) size: u64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SidecarRuntimeMetadata {
    pub(crate) component_version: String,
    pub(crate) protocol: u32,
    pub(crate) target: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsbipInstallResult {
    Ready,
    RebootRecommended,
}

pub(crate) struct ComponentDownloadSpec<'a> {
    pub(crate) url: &'a str,
    pub(crate) destination: &'a Path,
    pub(crate) expected_size: Option<u64>,
    pub(crate) max_size: u64,
    pub(crate) stage: &'a str,
    pub(crate) progress_start: u32,
    pub(crate) progress_span: u32,
    pub(crate) error_code: &'a str,
}

pub(crate) async fn download_component_asset(
    spec: ComponentDownloadSpec<'_>,
    progress: &ProgressReporter<'_>,
) -> Result<(), String> {
    report_progress(progress, spec.stage, spec.progress_start);
    let mut highest_progress = spec.progress_start;
    github_download::download_to_file_with_fallbacks(
        DownloadRequest {
            url: spec.url,
            destination: spec.destination,
            expected_size: spec.expected_size,
            max_size: Some(spec.max_size),
            user_agent: "foundation-sunshine-dualsense-component",
            connect_timeout: std::time::Duration::from_secs(10),
            response_timeout: DEFAULT_RESPONSE_TIMEOUT,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
            overall_timeout: Some(COMPONENT_DOWNLOAD_OVERALL_TIMEOUT),
        },
        |event| {
            if event.phase != DownloadAttemptPhase::Downloading || event.total == 0 {
                return;
            }
            let value = spec.progress_start
                + (event.downloaded.saturating_mul(spec.progress_span as u64) / event.total)
                    .min(spec.progress_span as u64) as u32;
            if value > highest_progress {
                highest_progress = value;
                report_progress(progress, spec.stage, value);
            }
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| format!("{}: download failed: {error}", spec.error_code))
}

pub(crate) fn component_root() -> PathBuf {
    PathBuf::from(crate::sunshine::get_sunshine_install_path())
        .join("tools")
        .join("sunshine-ds5-component")
}

pub(crate) fn active_dir() -> PathBuf {
    component_root().join("active")
}

pub(crate) fn sidecar_path() -> PathBuf {
    active_dir().join(SIDECAR_EXE)
}

pub(crate) fn sidecar_package_manifest_path() -> PathBuf {
    PathBuf::from(crate::sunshine::get_sunshine_install_path())
        .join("tools")
        .join(SIDECAR_PACKAGE_MANIFEST)
}

pub(crate) fn manually_placed_sidecar_package() -> Option<PathBuf> {
    let sunshine_root = PathBuf::from(crate::sunshine::get_sunshine_install_path());
    [
        sunshine_root.join(SIDECAR_PACKAGE_ASSET),
        sunshine_root.join("tools").join(SIDECAR_PACKAGE_ASSET),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub(crate) fn validate_sidecar_package_manifest(
    manifest: SidecarPackageManifest,
) -> Result<SidecarPackageManifest, String> {
    let valid_digest =
        manifest.sha256.len() == 64 && manifest.sha256.bytes().all(|byte| byte.is_ascii_hexdigit());
    if manifest.schema != 1
        || manifest.component_version != COMPONENT_VERSION
        || manifest.protocol != PROTOCOL_VERSION
        || manifest.target != SIDECAR_PACKAGE_TARGET
        || manifest.license != SIDECAR_PACKAGE_LICENSE
        || manifest.asset_name != SIDECAR_PACKAGE_ASSET
        || !valid_digest
        || manifest.size == 0
        || manifest.size > MAX_SIDECAR_PACKAGE_BYTES
    {
        return Err("DS5-PKG-002: the DualSense package manifest is invalid".to_string());
    }
    if !manifest.download_url.is_empty() {
        let url = url::Url::parse(&manifest.download_url)
            .map_err(|_| "DS5-PKG-002: the DualSense package URL is invalid".to_string())?;
        let valid_url = url.scheme() == "https"
            && url.host_str() == Some("github.com")
            && url
                .path()
                .starts_with("/AlkaidLab/foundation-sunshine/releases/download/")
            && url
                .path_segments()
                .and_then(Iterator::last)
                .is_some_and(|name| name == SIDECAR_PACKAGE_ASSET);
        if !valid_url {
            return Err("DS5-PKG-002: the DualSense package URL is not trusted".to_string());
        }
    }
    Ok(manifest)
}

pub(crate) fn read_sidecar_package_manifest(path: &Path) -> Result<SidecarPackageManifest, String> {
    let mut file = File::open(path).map_err(|error| {
        format!(
            "DS5-PKG-002: the DualSense package manifest is missing ({}): {error}",
            path.display()
        )
    })?;
    let mut contents = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_SIDECAR_PACKAGE_MANIFEST_BYTES + 1)
        .read_to_end(&mut contents)
        .map_err(|error| {
            format!(
                "DS5-PKG-002: unable to read the DualSense package manifest ({}): {error}",
                path.display()
            )
        })?;
    if contents.len() as u64 > MAX_SIDECAR_PACKAGE_MANIFEST_BYTES {
        return Err("DS5-PKG-002: the DualSense package manifest is too large".to_string());
    }
    let manifest = serde_json::from_slice(&contents)
        .map_err(|error| format!("DS5-PKG-002: invalid DualSense package manifest: {error}"))?;
    validate_sidecar_package_manifest(manifest)
}

pub(crate) fn sidecar_package_manifest() -> Result<SidecarPackageManifest, String> {
    read_sidecar_package_manifest(&sidecar_package_manifest_path())
}

pub(crate) fn classify_local_component_packages<P>(
    packages: impl IntoIterator<Item = P>,
) -> Result<LocalComponentPackages, String>
where
    P: AsRef<Path>,
{
    let packages = packages.into_iter().collect::<Vec<_>>();
    if packages.is_empty() {
        return Ok(LocalComponentPackages::default());
    }
    let manifest_path = sidecar_package_manifest_path();
    let sidecar_manifest = if manifest_path.try_exists().map_err(|error| {
        format!("DS5-PKG-002: unable to inspect the DualSense package manifest: {error}")
    })? {
        Some(sidecar_package_manifest()?)
    } else {
        None
    };
    let mut classified = LocalComponentPackages::default();

    for package in packages {
        let package = package.as_ref();
        let digest = sha256_file(package)?;
        let Some(kind) = local_component_kind(
            &digest,
            sidecar_manifest
                .as_ref()
                .map(|manifest| manifest.sha256.as_str()),
        ) else {
            return Err(
                "DS5-PKG-005: a selected local package does not match any component pinned by this Sunshine build"
                    .to_string(),
            );
        };
        let (slot, component) = match kind {
            LocalComponentKind::Sidecar => (&mut classified.sidecar, "Sunshine Sidecar"),
            LocalComponentKind::Hidmaestro => (&mut classified.hidmaestro, "HIDMaestro"),
            LocalComponentKind::Usbip => (&mut classified.usbip, "USB/IP"),
        };
        if slot.replace(package.to_path_buf()).is_some() {
            return Err(format!(
                "DS5-PKG-005: more than one local {component} package was selected"
            ));
        }
    }

    Ok(classified)
}

pub(crate) fn local_component_kind(
    digest: &str,
    sidecar_digest: Option<&str>,
) -> Option<LocalComponentKind> {
    if digest.eq_ignore_ascii_case(HIDMAESTRO_SHA256) {
        Some(LocalComponentKind::Hidmaestro)
    } else if digest.eq_ignore_ascii_case(USBIP_SHA256) {
        Some(LocalComponentKind::Usbip)
    } else if sidecar_digest.is_some_and(|expected| digest.eq_ignore_ascii_case(expected)) {
        Some(LocalComponentKind::Sidecar)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn local_component_package_handoff_path(
    root: &Path,
    token: uuid::Uuid,
    index: usize,
) -> PathBuf {
    root.join(format!("handoff-{token}-{index}.partial"))
}

pub(crate) fn purge_stale_handoff_packages(root: &Path, current_packages: &[&Path]) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if current_packages.iter().any(|current| **current == path) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with("handoff-")
            && (name.ends_with(".partial") || name.ends_with(".partial.zip"))
        {
            let _ = fs::remove_file(path);
        }
    }
}

pub(crate) fn installed_component_manifest() -> Option<InstalledComponentManifest> {
    let contents = fs::read(active_dir().join("component.json")).ok()?;
    serde_json::from_slice(&contents).ok()
}

pub(crate) fn component_update_available(manifest: Option<&InstalledComponentManifest>) -> bool {
    manifest.is_some_and(|manifest| manifest.component_version != COMPONENT_VERSION)
}

pub(crate) fn component_matches_current_runtime(
    manifest: Option<&InstalledComponentManifest>,
    genshin_compatibility_available: bool,
    audio_policy_violation_available: bool,
) -> bool {
    let Some(manifest) = manifest else {
        return false;
    };
    manifest.component_version == COMPONENT_VERSION
        && manifest.hidmaestro_version == HIDMAESTRO_VERSION
        && manifest.sha256.eq_ignore_ascii_case(HIDMAESTRO_SHA256)
        && manifest.protocol == PROTOCOL_VERSION
        && manifest.sidecar_file == SIDECAR_EXE
        && genshin_compatibility_available
        && audio_policy_violation_available
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, String> {
    let mut input = File::open(path)
        .map_err(|error| format!("DS5-PKG-002: unable to open component file: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("DS5-PKG-002: unable to hash component file: {error}"))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

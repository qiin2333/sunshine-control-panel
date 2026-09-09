//! Recoverable runtime replacement, settled against Core's persisted version.

use super::{ComponentManifest, RUNTIME_FILE, validate_version};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const PENDING: &str = "pending-runtime";
const FILES: [&str; 2] = [RUNTIME_FILE, "component.json"];

#[derive(Serialize, Deserialize)]
struct Record {
    manifest: ComponentManifest,
    previous: [bool; 2],
}

fn error() -> String {
    "HDR-PKG-004: component transaction could not be completed; recovery is required".to_string()
}

pub(super) fn copy_bounded(source: &Path, target: &Path, maximum: u64) -> Result<(), String> {
    let input = fs::File::open(source).map_err(|_| error())?;
    let mut output = fs::File::create(target).map_err(|_| error())?;
    let size = std::io::copy(&mut input.take(maximum + 1), &mut output).map_err(|_| error())?;
    if size > maximum {
        return Err(error());
    }
    output.sync_all().map_err(|_| error())
}

fn directory(root: &Path) -> Result<PathBuf, String> {
    let expected = root.canonicalize().map_err(|_| error())?.join(PENDING);
    let actual = expected.canonicalize().map_err(|_| error())?;
    if actual != expected {
        return Err(error());
    }
    Ok(actual)
}

pub(super) fn pending(root: &Path) -> bool {
    root.join(PENDING).exists()
}

/// Persist rollback data before replacing either active file.
pub(super) fn prepare(root: &Path, manifest: &ComponentManifest) -> Result<(), String> {
    let path = root.join(PENDING);
    fs::create_dir(&path).map_err(|_| error())?;
    let result = (|| {
        let mut previous = [false; 2];
        for (index, name) in FILES.iter().enumerate() {
            let source = root.join(name);
            previous[index] = source.exists();
            if previous[index] {
                if source.canonicalize().map_err(|_| error())?.parent() != Some(root) {
                    return Err(error());
                }
                let maximum = if *name == RUNTIME_FILE {
                    super::MAX_RUNTIME_BYTES
                } else {
                    64 * 1024
                };
                copy_bounded(&source, &path.join(name), maximum)?;
            }
        }
        let record = Record {
            manifest: manifest.clone(),
            previous,
        };
        let temporary = path.join("transaction.json.tmp");
        let mut file = fs::File::create(&temporary).map_err(|_| error())?;
        file.write_all(&serde_json::to_vec(&record).map_err(|_| error())?)
            .map_err(|_| error())?;
        file.sync_all().map_err(|_| error())?;
        drop(file);
        fs::rename(temporary, path.join("transaction.json")).map_err(|_| error())
    })();
    if result.is_err() {
        // 此时尚未动过活动文件，失败的备份可以删除。
        let _ = fs::remove_dir_all(&path);
    }
    result
}

/// Keep the installed version only when Core persisted that exact version.
pub(super) fn settle(root: &Path, configured_version: Option<&str>) -> Result<(), String> {
    if !pending(root) {
        return Ok(());
    }
    let path = directory(root)?;
    let record_path = path.join("transaction.json");
    if !record_path.exists() {
        // 备份尚未完成，或已完成恢复但清理中断；两种情况都不再修改活动文件。
        return fs::remove_dir_all(path).map_err(|_| error());
    }
    let mut bytes = Vec::new();
    fs::File::open(record_path)
        .and_then(|file| file.take(64 * 1024 + 1).read_to_end(&mut bytes))
        .map_err(|_| error())?;
    if bytes.len() > 64 * 1024 {
        return Err(error());
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| error())?;
    if configured_version != Some(record.manifest.component_id.as_str())
        || !validate_version(root, &record.manifest)
    {
        for (index, name) in FILES.iter().enumerate() {
            let target = root.join(name);
            if record.previous[index] {
                let source = path.join(name);
                if source.canonicalize().map_err(|_| error())?.parent() != Some(path.as_path()) {
                    return Err(error());
                }
                // 先删除目的文件，避免跟随意外出现的文件符号链接；失败时保留恢复资料。
                if target.symlink_metadata().is_ok() {
                    fs::remove_file(&target).map_err(|_| error())?;
                }
                let maximum = if *name == RUNTIME_FILE {
                    super::MAX_RUNTIME_BYTES
                } else {
                    64 * 1024
                };
                copy_bounded(&source, &target, maximum)?;
            } else if target.symlink_metadata().is_ok() {
                fs::remove_file(target).map_err(|_| error())?;
            }
        }
    }
    // 先撤销恢复指令，再清理备份；清理中断后不能再次使用残缺备份覆盖活动文件。
    fs::remove_file(path.join("transaction.json")).map_err(|_| error())?;
    fs::remove_dir_all(path).map_err(|_| error())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_replacement_restores_both_previous_files() {
        let root = std::env::temp_dir().join(format!("hdr-transaction-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        fs::write(root.join(RUNTIME_FILE), b"previous runtime").unwrap();
        fs::write(root.join("component.json"), b"previous manifest").unwrap();
        let manifest = ComponentManifest {
            schema: 1,
            component_id: "new-version".into(),
            runtime_sha256: "b".repeat(64),
        };
        prepare(&root, &manifest).unwrap();
        fs::write(root.join(RUNTIME_FILE), b"new runtime").unwrap();
        fs::remove_file(root.join("component.json")).unwrap();
        settle(&root, Some("previous-version")).unwrap();
        assert_eq!(
            fs::read(root.join(RUNTIME_FILE)).unwrap(),
            b"previous runtime"
        );
        assert_eq!(
            fs::read(root.join("component.json")).unwrap(),
            b"previous manifest"
        );
        assert!(!pending(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_first_install_restores_absence() {
        let root = std::env::temp_dir().join(format!("hdr-transaction-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let manifest = ComponentManifest {
            schema: 1,
            component_id: "new-version".into(),
            runtime_sha256: "b".repeat(64),
        };
        prepare(&root, &manifest).unwrap();
        fs::write(root.join(RUNTIME_FILE), b"partial install").unwrap();
        settle(&root, None).unwrap();
        assert!(!root.join(RUNTIME_FILE).exists());
        assert!(!pending(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_cleanup_does_not_replay_a_partial_backup() {
        let root = std::env::temp_dir().join(format!("hdr-cleanup-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        fs::create_dir(root.join(PENDING)).unwrap();
        fs::write(root.join(RUNTIME_FILE), b"settled runtime").unwrap();
        fs::write(
            root.join(PENDING).join("component.json"),
            b"leftover backup",
        )
        .unwrap();
        settle(&root, None).unwrap();
        assert_eq!(
            fs::read(root.join(RUNTIME_FILE)).unwrap(),
            b"settled runtime"
        );
        assert!(!pending(&root));
        fs::remove_dir_all(root).unwrap();
    }
}

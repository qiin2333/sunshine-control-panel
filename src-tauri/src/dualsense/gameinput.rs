//! Explicit, device-free repair of the historical physical DualSense mapping.

/// Accept only the fixed identifier format, never a path passed to an elevated process.
pub(crate) fn validate_backup_id(id: &str) -> Result<(), String> {
    if id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("DS5-MAP-001: invalid mapping backup ID".to_string())
    }
}

#[cfg(target_os = "windows")]
/// Receive exactly one identifier, leaving later bytes to the helper's disconnect watcher.
pub(crate) async fn receive_backup_id<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<Option<String>, String> {
    use tokio::io::AsyncReadExt;
    let mut bytes = [0u8; 32];
    reader
        .read_exact(&mut bytes)
        .await
        .map_err(|e| format!("DS5-MAP-001: missing restore backup ID: {e}"))?;
    let id = String::from_utf8(bytes.to_vec())
        .map_err(|_| "DS5-MAP-001: invalid backup ID encoding".to_string())?;
    validate_backup_id(&id)?;
    Ok(Some(id))
}

/// Run the verified component's device-free command and retain structured recovery results.
pub(crate) async fn run_mapping_command(
    action: &'static str,
    backup_id: Option<String>,
) -> Result<serde_json::Value, String> {
    if !matches!(action, "check" | "repair" | "restore")
        || (action == "restore") != backup_id.is_some()
    {
        return Err("DS5-MAP-001: invalid mapping operation".to_string());
    }
    if let Some(id) = &backup_id {
        validate_backup_id(id)?;
    }
    #[cfg(target_os = "windows")]
    {
        tokio::task::spawn_blocking(move || {
            // Verification works even when USB/IP or the UMDF driver is absent.
            let executable = super::packages::verified_sidecar_path()?;
            let mut command = std::process::Command::new(&executable);
            command.args(["--gameinput", action]);
            if let Some(id) = backup_id {
                command.args(["--backup-id", &id]);
            }
            command.current_dir(
                executable
                    .parent()
                    .ok_or("DS5-MAP-001: invalid component path")?,
            );
            let output = super::probe::run_with_timeout(
                &mut command,
                std::time::Duration::from_secs(30),
                "DS5-MAP-001: mapping operation timed out; check its recovery status",
                true,
            )?;
            parse_mapping_output(output.status.success(), &output.stdout, &output.stderr)
        })
        .await
        .map_err(|e| format!("DS5-MAP-001: mapping task failed: {e}"))?
    }
    #[cfg(not(target_os = "windows"))]
    Err("DS5-MAP-001: mapping repair is only supported on Windows".to_string())
}

#[cfg(any(target_os = "windows", test))]
/// Bound reports to the administrator IPC envelope without discarding actionable failures.
fn parse_mapping_output(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<serde_json::Value, String> {
    // Leave room for the existing 64 KiB administrator IPC envelope.
    if stdout.len() > 48 * 1024 {
        return Err("DS5-MAP-001: mapping report exceeds the size limit".to_string());
    }
    let data: serde_json::Value = serde_json::from_slice(stdout).map_err(|_| {
        format!(
            "DS5-MAP-001: invalid mapping report: {}",
            String::from_utf8_lossy(stderr).trim()
        )
    })?;
    if data
        .get("state")
        .and_then(serde_json::Value::as_str)
        .is_none()
    {
        return Err("DS5-MAP-001: mapping report has no state".to_string());
    }
    // A failed apply may still return a valid recovery report and backup ID.
    if !success && data.get("detail").is_none_or(serde_json::Value::is_null) {
        return Err("DS5-MAP-001: mapping command failed without a diagnosis".to_string());
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_ids_are_fixed_identifiers_not_paths() {
        assert!(validate_backup_id("00112233445566778899aabbccddeeff").is_ok());
        for invalid in [
            "",
            "../outside",
            "00112233-4455-6677-8899-aabbccddeeff",
            "g0112233445566778899aabbccddeeff",
        ] {
            assert!(validate_backup_id(invalid).is_err());
        }
    }
    #[test]
    fn report_keeps_failed_repairs_and_rejects_invalid_results() {
        let recovery = br#"{"state":"recovery_required","backup_id":"00112233445566778899aabbccddeeff","detail":"conflict"}"#;
        assert_eq!(
            parse_mapping_output(false, recovery, b"").unwrap()["state"],
            "recovery_required"
        );
        assert!(parse_mapping_output(true, b"{}", b"").is_err());
        assert!(parse_mapping_output(false, br#"{"state":"repaired"}"#, b"").is_err());
        assert!(parse_mapping_output(true, &vec![b' '; 49153], b"").is_err());
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn restore_handoff_reads_one_bounded_id_and_rejects_incomplete_payloads() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut writer, mut reader) = tokio::io::duplex(64);
        writer
            .write_all(b"00112233445566778899aabbccddeeff!")
            .await
            .unwrap();
        assert_eq!(
            receive_backup_id(&mut reader).await.unwrap().unwrap(),
            "00112233445566778899aabbccddeeff"
        );
        assert_eq!(reader.read_u8().await.unwrap(), b'!');
        for invalid in [b"short".as_slice(), &[b'g'; 32]] {
            let (mut writer, mut reader) = tokio::io::duplex(64);
            writer.write_all(invalid).await.unwrap();
            writer.shutdown().await.unwrap();
            assert!(receive_backup_id(&mut reader).await.is_err());
        }
    }
}

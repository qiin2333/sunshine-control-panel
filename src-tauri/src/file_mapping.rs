use crate::sunshine::{create_https_client, get_sunshine_url};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FileMappingInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub mode: String,
    pub allow_delete: bool,
    pub allow_execute: bool,
    pub follow_reparse_points: bool,
    pub max_file_size: u64,
    pub clients: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateMappingResponse {
    ok: bool,
    mapping: Option<FileMappingInfo>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListMappingsResponse {
    ok: bool,
    mappings: Option<Vec<FileMappingInfo>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeleteMappingResponse {
    ok: bool,
    error: Option<String>,
}

pub fn parse_quick_share_folder_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];

        if arg == "--quick-share-folder" {
            if let Some(path) = args.get(index + 1) {
                if !path.trim().is_empty() && !path.starts_with("--") {
                    out.push(path.clone());
                    index += 1;
                }
            }
        } else if let Some(path) = arg.strip_prefix("--quick-share-folder=") {
            if !path.trim().is_empty() {
                out.push(path.to_string());
            }
        }

        index += 1;
    }

    out
}

pub fn dispatch_cli_quick_share(paths: Vec<String>) {
    if paths.is_empty() {
        return;
    }

    tauri::async_runtime::spawn(async move {
        match quick_share_folder_internal(paths).await {
            Ok(mapping) => {
                let message = format!("已共享文件夹：{}\n{}", mapping.name, mapping.path);
                info!(
                    "file mapping quick share created: {} ({})",
                    mapping.name, mapping.path
                );
                show_quick_share_message("Sunshine 文件夹共享", &message, false);
            }
            Err(e) => {
                let message = format!("共享文件夹失败：\n{e}");
                warn!("file mapping quick share failed: {e}");
                show_quick_share_message("Sunshine 文件夹共享", &message, true);
            }
        }
    });
}

#[tauri::command]
pub async fn quick_share_folder(path: String) -> Result<FileMappingInfo, String> {
    quick_share_folder_internal(vec![path]).await
}

#[tauri::command]
pub async fn list_file_mappings() -> Result<Vec<FileMappingInfo>, String> {
    list_mappings().await
}

#[tauri::command]
pub async fn delete_file_mapping(id: String) -> Result<String, String> {
    delete_mapping(&id).await?;
    Ok("success".to_string())
}

#[tauri::command]
pub async fn update_file_mapping(
    id: String,
    patch: serde_json::Value,
) -> Result<FileMappingInfo, String> {
    update_mapping(&id, patch).await
}

#[tauri::command]
pub async fn install_file_mapping_menu() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        crate::shell_context_menu::install_file_mapping_menu()?;
        crate::desktop_settings::set_file_mapping_menu_enabled(true)?;
        Ok("success".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("file mapping shell menu is only supported on Windows".to_string())
    }
}

#[tauri::command]
pub async fn uninstall_file_mapping_menu() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        crate::shell_context_menu::uninstall_file_mapping_menu()?;
        crate::desktop_settings::set_file_mapping_menu_enabled(false)?;
        Ok("success".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("file mapping shell menu is only supported on Windows".to_string())
    }
}

async fn quick_share_folder_internal(paths: Vec<String>) -> Result<FileMappingInfo, String> {
    let first = paths
        .first()
        .ok_or_else(|| "no folder was selected".to_string())?;

    if paths.len() > 1 {
        return Err("only one folder can be shared at a time".to_string());
    }

    let path = canonicalize_directory(first)?;
    create_quick_share(&path).await
}

fn canonicalize_directory(path: &str) -> Result<PathBuf, String> {
    let raw = PathBuf::from(path);
    let canonical = std::fs::canonicalize(&raw).map_err(|e| {
        format!(
            "folder does not exist or cannot be accessed: {} ({e})",
            raw.display()
        )
    })?;
    if !canonical.is_dir() {
        return Err("the selected path is not a folder".to_string());
    }
    Ok(canonical)
}

async fn create_quick_share(path: &PathBuf) -> Result<FileMappingInfo, String> {
    let url = get_sunshine_url().await?;
    let endpoint = format!("{}/api/v1/file-mapping/mappings", url.trim_end_matches('/'));
    let client = create_https_client()?;
    let body = serde_json::json!({
        "path": path_for_sunshine_api(path),
    });

    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Sunshine mapping API failed: HTTP {status} {text}"));
    }

    let parsed: CreateMappingResponse = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse Sunshine mapping API response: {e}; body={text}"))?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Sunshine mapping API returned an error".to_string()));
    }

    parsed
        .mapping
        .ok_or_else(|| "Sunshine mapping API response did not include a mapping".to_string())
}

fn path_for_sunshine_api(path: &PathBuf) -> String {
    strip_windows_verbatim_prefix(&path.to_string_lossy())
}

fn strip_windows_verbatim_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{}", rest)
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

#[cfg(target_os = "windows")]
fn show_quick_share_message(title: &str, message: &str, is_error: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW,
    };
    use windows::core::PCWSTR;

    let title = to_wide_null(title);
    let message = to_wide_null(message);
    let icon = if is_error {
        MB_ICONERROR
    } else {
        MB_ICONINFORMATION
    };

    unsafe {
        let _ = MessageBoxW(
            Some(HWND(std::ptr::null_mut())),
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | icon,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_quick_share_message(_title: &str, _message: &str, _is_error: bool) {}

fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::strip_windows_verbatim_prefix;

    #[test]
    fn strips_windows_verbatim_disk_prefix() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\C:\Users\alice\Downloads"),
            r"C:\Users\alice\Downloads"
        );
    }

    #[test]
    fn strips_windows_verbatim_unc_prefix() {
        assert_eq!(
            strip_windows_verbatim_prefix(r"\\?\UNC\server\share\Downloads"),
            r"\\server\share\Downloads"
        );
    }
}

async fn list_mappings() -> Result<Vec<FileMappingInfo>, String> {
    let url = get_sunshine_url().await?;
    let endpoint = format!("{}/api/v1/file-mapping/mappings", url.trim_end_matches('/'));
    let client = create_https_client()?;

    let resp = client
        .get(endpoint)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Sunshine mapping API failed: HTTP {status} {text}"));
    }

    let parsed: ListMappingsResponse = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse Sunshine mapping API response: {e}; body={text}"))?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Sunshine mapping API returned an error".to_string()));
    }

    Ok(parsed.mappings.unwrap_or_default())
}

async fn delete_mapping(id: &str) -> Result<(), String> {
    let url = get_sunshine_url().await?;
    let endpoint = format!(
        "{}/api/v1/file-mapping/mappings/{}",
        url.trim_end_matches('/'),
        id
    );
    let client = create_https_client()?;

    let resp = client
        .delete(endpoint)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Sunshine mapping API failed: HTTP {status} {text}"));
    }

    let parsed: DeleteMappingResponse = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse Sunshine mapping API response: {e}; body={text}"))?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Sunshine mapping API returned an error".to_string()));
    }

    Ok(())
}

async fn update_mapping(id: &str, patch: serde_json::Value) -> Result<FileMappingInfo, String> {
    let url = get_sunshine_url().await?;
    let endpoint = format!(
        "{}/api/v1/file-mapping/mappings/{}",
        url.trim_end_matches('/'),
        id
    );
    let client = create_https_client()?;

    let resp = client
        .patch(endpoint)
        .json(&patch)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Sunshine mapping API failed: HTTP {status} {text}"));
    }

    let parsed: CreateMappingResponse = serde_json::from_str(&text)
        .map_err(|e| format!("failed to parse Sunshine mapping API response: {e}; body={text}"))?;
    if !parsed.ok {
        return Err(parsed
            .error
            .unwrap_or_else(|| "Sunshine mapping API returned an error".to_string()));
    }

    parsed
        .mapping
        .ok_or_else(|| "Sunshine mapping API response did not include a mapping".to_string())
}

use std::{
    fs,
    path::{Path, PathBuf},
};

use log::error;
use tauri::{AppHandle, Runtime};

use crate::{sunshine, tray::emit_message};

pub fn import_config<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match import_config_impl(&app_handle).await {
            Ok(()) => emit_message(
                &app_handle,
                "success",
                "Config imported. Restart Sunshine to apply every setting.",
            ),
            Err(e) if e == "cancelled" => {}
            Err(e) => {
                error!("Config import failed: {}", e);
                emit_message(&app_handle, "error", &e);
            }
        }
    });
}

pub fn export_config<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        match export_config_impl(&app_handle).await {
            Ok(path) => emit_message(
                &app_handle,
                "success",
                &format!("Config exported to {}", path.display()),
            ),
            Err(e) if e == "cancelled" => {}
            Err(e) => {
                error!("Config export failed: {}", e);
                emit_message(&app_handle, "error", &e);
            }
        }
    });
}

pub fn reset_config<R: Runtime>(app: &AppHandle<R>, title: &'static str, message: &'static str) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let app_handle = app.clone();
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNo)
        .show(move |confirmed| {
            if confirmed {
                reset_config_after_confirm(&app_handle);
            }
        });
}

fn reset_config_after_confirm<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();

    tauri::async_runtime::spawn(async move {
        let config_data = serde_json::Map::new();
        match sunshine::post_sunshine_config(&config_data).await {
            Ok(()) => emit_message(
                &app_handle,
                "success",
                "Config reset. Restart Sunshine to apply defaults.",
            ),
            Err(e) => {
                error!("Config reset failed: {}", e);
                emit_message(&app_handle, "error", &e);
            }
        }
    });
}

async fn import_config_impl<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let path = pick_config_file(app).await?;
    validate_config_path(&path)?;
    let content = fs::read_to_string(&path).map_err(|e| format!("read config failed: {}", e))?;
    let config_data = parse_config_content(&content)?;
    sunshine::post_sunshine_config(&config_data).await
}

async fn export_config_impl<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let content = fetch_config_for_export().await?;
    let path = pick_config_save_path(app).await?;
    fs::write(&path, content).map_err(|e| format!("write config failed: {}", e))?;
    Ok(path)
}

async fn pick_config_file<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Import Sunshine Config")
        .add_filter("Sunshine config", &["conf"]);

    let config_dir = sunshine::config_dir();
    if config_dir.exists() {
        dialog = dialog.set_directory(config_dir);
    }

    dialog.pick_file(move |file_path_opt| {
        let _ = tx.send(file_path_opt);
    });

    let file_path = rx
        .await
        .map_err(|_| "dialog channel error".to_string())?
        .ok_or_else(|| "cancelled".to_string())?;

    Ok(PathBuf::from(file_path.to_string()))
}

async fn pick_config_save_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::sync::oneshot;

    let (tx, rx) = oneshot::channel();
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Export Sunshine Config")
        .set_file_name("sunshine.conf")
        .add_filter("Sunshine config", &["conf"]);

    let config_dir = sunshine::config_dir();
    if config_dir.exists() {
        dialog = dialog.set_directory(config_dir);
    }

    dialog.save_file(move |file_path_opt| {
        let _ = tx.send(file_path_opt);
    });

    let file_path = rx
        .await
        .map_err(|_| "dialog channel error".to_string())?
        .ok_or_else(|| "cancelled".to_string())?;

    Ok(PathBuf::from(file_path.to_string()))
}

fn validate_config_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("config file does not exist".to_string());
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("conf") {
        return Err("only .conf files can be imported".to_string());
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|e| format!("read config metadata failed: {}", e))?;
    if metadata.file_type().is_symlink() {
        return Err("symlink config files are not allowed".to_string());
    }
    if !metadata.is_file() {
        return Err("selected config path is not a regular file".to_string());
    }
    if metadata.len() > 1024 * 1024 {
        return Err("config file is too large; max size is 1 MB".to_string());
    }
    Ok(())
}

fn parse_config_content(
    content: &str,
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    if content.trim().is_empty() {
        return Err("config file is empty".to_string());
    }
    if content.contains('\0') {
        return Err("config file contains invalid NUL bytes".to_string());
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut config_map = serde_json::Map::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.starts_with('#') || line.is_empty() {
            i += 1;
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if key.is_empty() {
                return Err("config contains an empty key".to_string());
            }

            let mut value = value.trim().to_string();
            if value.starts_with('[') && !value.ends_with(']') {
                i += 1;
                while i < lines.len() {
                    let next_line = lines[i].trim();
                    value.push('\n');
                    value.push_str(next_line);

                    if next_line.ends_with(']') {
                        break;
                    }
                    i += 1;
                }
            }

            config_map.insert(key.to_string(), serde_json::json!(value));
        } else {
            return Err(format!("invalid config line: {}", line));
        }

        i += 1;
    }

    if config_map.is_empty() {
        return Err("config file contains no settings".to_string());
    }

    Ok(config_map)
}

async fn fetch_config_for_export() -> Result<String, String> {
    let sunshine_url = sunshine::get_sunshine_url().await?;
    let config_url = format!("{}/api/config", sunshine_url.trim_end_matches('/'));
    let response = sunshine::send_https_request(|client| client.get(&config_url))
        .await
        .map_err(|e| format!("fetch config failed: {}", e))?;

    if !response.status().is_success() {
        return read_local_config_for_export();
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("parse config response failed: {}", e))?;

    config_json_to_conf(&json).or_else(|_| read_local_config_for_export())
}

fn read_local_config_for_export() -> Result<String, String> {
    let config_path = sunshine::config_dir().join("sunshine.conf");
    fs::read_to_string(&config_path).map_err(|e| format!("read local config failed: {}", e))
}

fn config_json_to_conf(json: &serde_json::Value) -> Result<String, String> {
    let object = json
        .as_object()
        .ok_or_else(|| "config response is not an object".to_string())?;
    let metadata_keys = [
        "status",
        "platform",
        "version",
        "display_devices",
        "adapters",
        "pair_name",
    ];

    let mut keys: Vec<&String> = object
        .keys()
        .filter(|key| !metadata_keys.contains(&key.as_str()))
        .collect();
    keys.sort();

    let mut lines = Vec::new();
    for key in keys {
        let Some(value) = object.get(key) else {
            continue;
        };

        let value = match value {
            serde_json::Value::String(value) => value.clone(),
            serde_json::Value::Bool(value) => value.to_string(),
            serde_json::Value::Number(value) => value.to_string(),
            serde_json::Value::Null
            | serde_json::Value::Array(_)
            | serde_json::Value::Object(_) => continue,
        };

        lines.push(format!("{} = {}", key, value));
    }

    if lines.is_empty() {
        return Err("config response contains no exportable settings".to_string());
    }

    lines.push(String::new());
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_export_sorts_settings_and_skips_metadata_and_complex_values() {
        let json = serde_json::json!({
            "version": "test-version",
            "z_setting": true,
            "a_setting": 42,
            "nested": { "ignored": true },
            "list": ["ignored"]
        });

        assert_eq!(
            config_json_to_conf(&json).unwrap(),
            "a_setting = 42\nz_setting = true\n"
        );
    }
}

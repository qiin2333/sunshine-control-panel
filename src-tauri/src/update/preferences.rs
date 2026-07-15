use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Runtime};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct UpdatePreferences {
    last_check_time: u64,
    include_prerelease: bool,
}

fn preferences_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("获取应用数据目录失败: {e}"))?;

    if !app_data_dir.exists() {
        fs::create_dir_all(&app_data_dir).map_err(|e| format!("创建应用数据目录失败: {e}"))?;
    }

    Ok(app_data_dir.join("update_preferences.json"))
}

fn load<R: Runtime>(app: &AppHandle<R>) -> UpdatePreferences {
    let path = match preferences_path(app) {
        Ok(path) => path,
        Err(error) => {
            warn!("⚠️ 获取更新偏好设置路径失败: {error}，使用默认偏好");
            return UpdatePreferences::default();
        }
    };

    if !path.exists() {
        return UpdatePreferences::default();
    }

    match fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str::<UpdatePreferences>(&content) {
            Ok(preferences) => {
                debug!(
                    "📨 已加载更新偏好: include_prerelease={}, last_check_time={}",
                    preferences.include_prerelease, preferences.last_check_time
                );
                preferences
            }
            Err(error) => {
                warn!("⚠️ 解析更新偏好设置失败: {error}，使用默认偏好");
                UpdatePreferences::default()
            }
        },
        Err(error) => {
            warn!("⚠️ 读取更新偏好设置失败: {error}，使用默认偏好");
            UpdatePreferences::default()
        }
    }
}

fn persist<R: Runtime>(app: &AppHandle<R>, preferences: &UpdatePreferences) {
    let path = match preferences_path(app) {
        Ok(path) => path,
        Err(error) => {
            warn!("⚠️ 获取更新偏好设置路径失败，无法保存: {error}");
            return;
        }
    };

    match serde_json::to_string_pretty(preferences) {
        Ok(json) => {
            if let Err(error) = fs::write(&path, json) {
                warn!("⚠️ 保存更新偏好设置失败: {error}");
            } else {
                debug!(
                    "💾 已保存更新偏好: include_prerelease={}, last_check_time={}",
                    preferences.include_prerelease, preferences.last_check_time
                );
            }
        }
        Err(error) => warn!("⚠️ 序列化更新偏好设置失败: {error}"),
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 确保更新偏好始终存在，包括 `--hidden` 的纯托盘代理启动。
fn ensure<R: Runtime>(app: &AppHandle<R>) -> bool {
    if app.try_state::<Arc<Mutex<UpdatePreferences>>>().is_some() {
        return true;
    }

    let _ = app.manage(Arc::new(Mutex::new(load(app))));
    let initialized = app.try_state::<Arc<Mutex<UpdatePreferences>>>().is_some();
    if !initialized {
        error!("❌ 更新偏好状态初始化失败");
    }
    initialized
}

pub(crate) fn init(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    if ensure(app) {
        Ok(())
    } else {
        Err(std::io::Error::other("failed to initialize update preferences").into())
    }
}

pub(crate) fn last_check_time<R: Runtime>(app: &AppHandle<R>) -> u64 {
    ensure(app);
    app.try_state::<Arc<Mutex<UpdatePreferences>>>()
        .map(|preferences| preferences.lock().unwrap().last_check_time)
        .unwrap_or(0)
}

pub(crate) fn save_last_check_time<R: Runtime>(app: &AppHandle<R>) {
    ensure(app);
    if let Some(preferences) = app.try_state::<Arc<Mutex<UpdatePreferences>>>() {
        let snapshot = {
            let mut preferences = preferences.lock().unwrap();
            preferences.last_check_time = current_timestamp();
            preferences.clone()
        };
        persist(app, &snapshot);
    }
}

pub(crate) fn include_prerelease<R: Runtime>(app: &AppHandle<R>) -> bool {
    ensure(app);
    app.try_state::<Arc<Mutex<UpdatePreferences>>>()
        .map(|preferences| preferences.lock().unwrap().include_prerelease)
        .unwrap_or(false)
}

pub(crate) fn set_include_prerelease<R: Runtime>(app: &AppHandle<R>, include: bool) {
    ensure(app);
    if let Some(preferences) = app.try_state::<Arc<Mutex<UpdatePreferences>>>() {
        let snapshot = {
            let mut preferences = preferences.lock().unwrap();
            preferences.include_prerelease = include;
            preferences.clone()
        };
        info!("📝 更新偏好设置: 包含预发布版本 = {include}");
        persist(app, &snapshot);
    }
}

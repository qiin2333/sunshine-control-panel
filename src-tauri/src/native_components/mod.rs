//! Fixed identities, execution hosts and distribution policies for native components.
pub(crate) mod install;
pub(crate) mod operation;
pub(crate) mod providers;

use serde::Serialize;
use std::path::PathBuf;

pub const NVIDIA_RTX_VIDEO_ID: &str = "alkaidlab.nvidia_rtx_video";
pub const RTX_VIDEO_BRIDGE: &str = "foundation_rtx_video_bridge.dll";
pub const RTX_HDR_RUNTIME: &str = "nvngx_truehdr.dll";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentHost {
    Gui,
    Core,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCategory {
    InputDiagnostics,
    HdrEnhanced,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct DistributionPolicy {
    pub bundled: bool,
    pub download: bool,
    pub local_import: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ComponentDescriptor {
    pub id: &'static str,
    pub category: ComponentCategory,
    pub host: ComponentHost,
    pub files: &'static [&'static str],
    pub distribution: DistributionPolicy,
    // 安装位置由宿主决定；远程元数据不能改变目录或执行宿主。
    #[serde(skip)]
    pub install_subdirectory: Option<&'static str>,
}

pub const COMPONENTS: &[ComponentDescriptor] = &[
    ComponentDescriptor {
        id: "alkaidlab.stylus",
        category: ComponentCategory::InputDiagnostics,
        host: ComponentHost::Gui,
        files: &["alkaidlab-plugin-stylus.dll"],
        distribution: DistributionPolicy {
            bundled: true,
            download: false,
            local_import: false,
        },
        install_subdirectory: None,
    },
    ComponentDescriptor {
        id: NVIDIA_RTX_VIDEO_ID,
        category: ComponentCategory::HdrEnhanced,
        host: ComponentHost::Core,
        files: &[RTX_HDR_RUNTIME],
        // NVIDIA 运行库不随 Panel 分发，也不提供远程下载，只允许用户明确导入本地文件。
        distribution: DistributionPolicy {
            bundled: false,
            download: false,
            local_import: true,
        },
        install_subdirectory: Some("hdr_enhanced/nvidia_rtx_video"),
    },
];

pub fn descriptor(id: &str) -> Option<&'static ComponentDescriptor> {
    COMPONENTS.iter().find(|component| component.id == id)
}

pub fn core_component_root(id: &str) -> Option<PathBuf> {
    let component = descriptor(id)?;
    if component.host != ComponentHost::Core {
        return None;
    }
    Some(
        crate::sunshine::install_dir()
            .join("tools")
            .join(component.install_subdirectory?),
    )
}

#[tauri::command]
pub fn list_native_components() -> Vec<ComponentDescriptor> {
    COMPONENTS.to_vec()
}

fn require_hdr_component(id: &str) -> Result<(), String> {
    if descriptor(id)
        .is_some_and(|item| item.id == NVIDIA_RTX_VIDEO_ID && item.host == ComponentHost::Core)
    {
        Ok(())
    } else {
        Err("COMPONENT-UNKNOWN: unsupported component operation".to_string())
    }
}

#[tauri::command]
pub async fn native_component_recover(
    component_id: String,
) -> Result<providers::nvidia_rtx_hdr::RtxHdrComponentStatus, String> {
    require_hdr_component(&component_id)?;
    let _operation = operation::COMPONENT_OPERATION
        .try_lock()
        .map_err(|_| "HDR-OP-001: another operation is running".to_string())?;
    providers::nvidia_rtx_hdr::rtx_hdr_recover().await
}

#[tauri::command]
pub async fn native_component_get_status(
    component_id: String,
) -> Result<providers::nvidia_rtx_hdr::RtxHdrComponentStatus, String> {
    require_hdr_component(&component_id)?;
    providers::nvidia_rtx_hdr::rtx_hdr_get_status().await
}

#[tauri::command]
pub async fn native_component_import(
    component_id: String,
    sources: std::collections::BTreeMap<String, String>,
) -> Result<providers::nvidia_rtx_hdr::RtxHdrComponentStatus, String> {
    require_hdr_component(&component_id)?;
    if sources.len() != 1 {
        return Err("COMPONENT-FILES: expected exactly one runtime file".to_string());
    }
    let runtime = sources
        .get(RTX_HDR_RUNTIME)
        .ok_or("COMPONENT-FILES: missing runtime file")?;
    providers::nvidia_rtx_hdr::rtx_hdr_install(runtime.clone()).await
}

#[tauri::command]
pub async fn native_component_remove(
    component_id: String,
) -> Result<providers::nvidia_rtx_hdr::RtxHdrComponentStatus, String> {
    require_hdr_component(&component_id)?;
    providers::nvidia_rtx_hdr::rtx_hdr_uninstall().await
}

#[tauri::command]
pub async fn hdr_enhanced_select_backend(
    component_id: String,
    enabled: bool,
) -> Result<providers::nvidia_rtx_hdr::RtxHdrComponentStatus, String> {
    require_hdr_component(&component_id)?;
    providers::nvidia_rtx_hdr::rtx_hdr_set_enabled(enabled).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_fixed_components_can_be_resolved() {
        assert!(descriptor("unknown.dll").is_none());
        assert!(descriptor("../alkaidlab.nvidia_rtx_video").is_none());
        assert_eq!(
            descriptor("alkaidlab.stylus").unwrap().host,
            ComponentHost::Gui
        );
    }

    #[test]
    fn hdr_is_core_owned_and_import_only() {
        let hdr = descriptor(NVIDIA_RTX_VIDEO_ID).unwrap();
        assert_eq!(hdr.category, ComponentCategory::HdrEnhanced);
        // 分类是通用能力，组件 ID 仍然指向特定后端，不能据此混用厂商 DLL。
        assert_ne!(hdr.id, "alkaidlab.hdr_enhanced");
        assert_eq!(hdr.host, ComponentHost::Core);
        assert_eq!(hdr.files, &[RTX_HDR_RUNTIME]);
        assert!(hdr.distribution.local_import);
        assert!(!hdr.distribution.download);
        assert!(!hdr.distribution.bundled);
        assert!(core_component_root("alkaidlab.stylus").is_none());
    }
}

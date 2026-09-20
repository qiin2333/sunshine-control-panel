use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct EnhancementComponentStatus {
    pub component_id: String,
    pub enabled: bool,
    pub download_available: bool,
    pub state: String,
    pub installed: bool,
    pub ready: bool,
    pub in_use: bool,
    pub maintenance: bool,
    pub host_supported: bool,
    pub adapter_present: bool,
    pub vc_runtime_present: bool,
    pub runtime_present: bool,
    pub configured: bool,
    pub managed_path: String,
    pub runtime_sha256: String,
}

pub(crate) mod nvidia_dlssnr;
pub(crate) mod nvidia_rtx_hdr;

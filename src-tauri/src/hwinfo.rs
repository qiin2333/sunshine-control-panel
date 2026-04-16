// HWiNFO 共享内存读取模块

#[cfg(target_os = "windows")]
use windows::Win32::System::Memory::*;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::*;

use serde::{Deserialize, Serialize};

// ─── HWiNFO Shared Memory 常量 ───

#[cfg(target_os = "windows")]
const HWINFO_SENSORS_STRING_LEN: usize = 128;
#[cfg(target_os = "windows")]
const HWINFO_UNIT_STRING_LEN: usize = 16;

// ─── HWiNFO 传感器类型 ───

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SensorReadingType {
    None = 0,
    Temp = 1,
    Voltage = 2,
    Fan = 3,
    Current = 4,
    Power = 5,
    Clock = 6,
    Usage = 7,
    Other = 8,
}

impl From<u32> for SensorReadingType {
    fn from(val: u32) -> Self {
        match val {
            0 => SensorReadingType::None,
            1 => SensorReadingType::Temp,
            2 => SensorReadingType::Voltage,
            3 => SensorReadingType::Fan,
            4 => SensorReadingType::Current,
            5 => SensorReadingType::Power,
            6 => SensorReadingType::Clock,
            7 => SensorReadingType::Usage,
            _ => SensorReadingType::Other,
        }
    }
}

// ─── 数据结构 ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwInfoSensor {
    pub id: u32,
    pub instance: u32,
    pub name_original: String,
    pub name_user: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwInfoReading {
    pub reading_type: SensorReadingType,
    pub sensor_index: u32,
    pub id: u32,
    pub label_original: String,
    pub label_user: String,
    pub unit: String,
    pub value: f64,
    pub value_min: f64,
    pub value_max: f64,
    pub value_avg: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwInfoData {
    pub version: u32,
    pub poll_time: u32,
    pub sensors: Vec<HwInfoSensor>,
    pub readings: Vec<HwInfoReading>,
}

// ─── 共享内存头结构 ───
// 参考 HWiNFO SDK: _HWiNFO_SENSORS_SHARED_MEM2
//
// Offset  Size  Field
// 0       4     dwSignature ('HWiS')
// 4       4     dwVersion
// 8       4     dwRevision
// 12      8     poll_time (INT64)
// 20      4     dwOffsetOfSensorSection
// 24      4     dwSizeOfSensorElement
// 28      4     dwNumSensorElements
// 32      4     dwOffsetOfReadingSection
// 36      4     dwSizeOfReadingElement
// 40      4     dwNumReadingElements
//
// Sensor Element (每个至少 ~260+ 字节):
// 0       4     dwSensorID
// 4       4     dwSensorInst
// 8       128   szSensorNameOrig
// 136     128   szSensorNameUser
//
// Reading Element (每个至少 ~560+ 字节):
// 0       4     tReading (SensorReadingType)
// 4       4     dwSensorIndex
// 8       4     dwReadingID
// 12      128   szLabelOrig
// 140     128   szLabelUser
// 268     16    szUnit
// 284     8     Value (double)
// 292     8     ValueMin (double)
// 300     8     ValueMax (double)
// 308     8     ValueAvg (double)

#[cfg(target_os = "windows")]
fn read_string_from_ptr(ptr: *const u8, max_len: usize) -> String {
    let mut bytes = Vec::with_capacity(max_len);
    for i in 0..max_len {
        let b = unsafe { *ptr.add(i) };
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).to_string()
}

#[cfg(target_os = "windows")]
fn read_u32(ptr: *const u8) -> u32 {
    unsafe { *(ptr as *const u32) }
}

#[cfg(target_os = "windows")]
fn read_i64(ptr: *const u8) -> i64 {
    unsafe { *(ptr as *const i64) }
}

#[cfg(target_os = "windows")]
fn read_f64(ptr: *const u8) -> f64 {
    unsafe { *(ptr as *const f64) }
}

/// 从 HWiNFO 共享内存读取所有传感器数据
#[cfg(target_os = "windows")]
pub fn read_hwinfo_shared_memory() -> Result<HwInfoData, String> {
    use windows::core::s;

    unsafe {
        let handle = OpenFileMappingA(FILE_MAP_READ.0, false, s!("Global\\HWiNFO_SENS_SM2"))
            .map_err(|e| format!("无法打开 HWiNFO 共享内存: {}。请确保 HWiNFO 正在运行且已启用 Shared Memory Support", e))?;

        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
        let ptr = view.Value as *const u8;
        if ptr.is_null() {
            let _ = CloseHandle(handle);
            return Err("无法映射 HWiNFO 共享内存".to_string());
        }

        // 读取头结构
        let signature = read_u32(ptr);
        if signature != 0x53695748 {
            let _ = UnmapViewOfFile(view);
            let _ = CloseHandle(handle);
            return Err(format!("HWiNFO 共享内存签名无效: 0x{:08X}", signature));
        }

        let version = read_u32(ptr.add(4));
        let _revision = read_u32(ptr.add(8));
        let poll_time = read_i64(ptr.add(12)) as u32;

        let sensor_offset = read_u32(ptr.add(20)) as usize;
        let sensor_size = read_u32(ptr.add(24)) as usize;
        let sensor_count = read_u32(ptr.add(28)) as usize;

        let reading_offset = read_u32(ptr.add(32)) as usize;
        let reading_size = read_u32(ptr.add(36)) as usize;
        let reading_count = read_u32(ptr.add(40)) as usize;

        // 读取传感器
        let mut sensors = Vec::with_capacity(sensor_count);
        for i in 0..sensor_count {
            let base = ptr.add(sensor_offset + i * sensor_size);
            sensors.push(HwInfoSensor {
                id: read_u32(base),
                instance: read_u32(base.add(4)),
                name_original: read_string_from_ptr(base.add(8), HWINFO_SENSORS_STRING_LEN),
                name_user: read_string_from_ptr(base.add(8 + HWINFO_SENSORS_STRING_LEN), HWINFO_SENSORS_STRING_LEN),
            });
        }

        // 读取传感器读数
        let mut readings = Vec::with_capacity(reading_count);
        for i in 0..reading_count {
            let base = ptr.add(reading_offset + i * reading_size);
            readings.push(HwInfoReading {
                reading_type: SensorReadingType::from(read_u32(base)),
                sensor_index: read_u32(base.add(4)),
                id: read_u32(base.add(8)),
                label_original: read_string_from_ptr(base.add(12), HWINFO_SENSORS_STRING_LEN),
                label_user: read_string_from_ptr(base.add(12 + HWINFO_SENSORS_STRING_LEN), HWINFO_SENSORS_STRING_LEN),
                unit: read_string_from_ptr(base.add(12 + HWINFO_SENSORS_STRING_LEN * 2), HWINFO_UNIT_STRING_LEN),
                value: read_f64(base.add(12 + HWINFO_SENSORS_STRING_LEN * 2 + HWINFO_UNIT_STRING_LEN)),
                value_min: read_f64(base.add(12 + HWINFO_SENSORS_STRING_LEN * 2 + HWINFO_UNIT_STRING_LEN + 8)),
                value_max: read_f64(base.add(12 + HWINFO_SENSORS_STRING_LEN * 2 + HWINFO_UNIT_STRING_LEN + 16)),
                value_avg: read_f64(base.add(12 + HWINFO_SENSORS_STRING_LEN * 2 + HWINFO_UNIT_STRING_LEN + 24)),
            });
        }

        let _ = UnmapViewOfFile(view);
        let _ = CloseHandle(handle);

        Ok(HwInfoData {
            version,
            poll_time,
            sensors,
            readings,
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn read_hwinfo_shared_memory() -> Result<HwInfoData, String> {
    Err("HWiNFO 仅支持 Windows".to_string())
}

// ─── Tauri 命令 ───

/// 获取 HWiNFO 传感器列表（传感器+读数名称，不含实时值）
#[tauri::command]
pub fn hwinfo_get_sensors() -> Result<HwInfoData, String> {
    read_hwinfo_shared_memory()
}

/// 获取指定传感器读数的实时值
/// reading_ids: 要查询的读数索引列表，空则返回全部
#[tauri::command]
pub fn hwinfo_get_readings(reading_ids: Vec<u32>) -> Result<Vec<HwInfoReading>, String> {
    let data = read_hwinfo_shared_memory()?;
    if reading_ids.is_empty() {
        Ok(data.readings)
    } else {
        Ok(data.readings.into_iter()
            .enumerate()
            .filter(|(i, _)| reading_ids.contains(&(*i as u32)))
            .map(|(_, r)| r)
            .collect())
    }
}

/// 检查 HWiNFO 共享内存是否可用
#[tauri::command]
pub fn hwinfo_check_available() -> Result<bool, String> {
    match read_hwinfo_shared_memory() {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

use serde::{Deserialize, Serialize};

// ─── Product Selection ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    #[serde(rename = "Brand")]
    pub brand: String,
    #[serde(rename = "Type")]
    pub product_type: String,
    #[serde(rename = "Fac")]
    pub fac: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub name: String,
    pub index: i32,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BaseData {
    #[serde(default)]
    pub brands: Vec<Item>,
    #[serde(rename = "types", default)]
    pub product_types: Vec<Item>,
    #[serde(default)]
    pub factories: Vec<Item>,
}

// ─── Code Set (SN components) ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSet {
    pub brand_code: String,
    pub type_code: String,
    pub fac_code: String,
    pub year_code: String,
    pub mon_code: String,
    pub seq_code: String,
}

// ─── Execution Data ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteData {
    pub date_str: String,
    #[serde(rename = "type")]
    pub product_type: String,
    pub prefix_str: String,
    pub curret_seq_no: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutDataList {
    pub exe_data_list: Vec<ExecuteData>,
}

// ─── Device Info ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub imei: String,
    pub iccid: String,
    pub sn: String,
    pub sw_version: String,
    pub device_name: String,
    pub timestamp: String,
    pub activated: bool,
}

// CSV record exported for device production trace

// ─── REST API Response ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceActivateInfo {
    pub firmware_version: String,
    pub imei: String,
    pub iccid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceNameData {
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialNumberData {
    pub serial_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationStatus {
    pub valid: bool,
    pub level: i32,
    pub activate_time: i64,
    pub expires_time: i64,
    pub status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyData {}

// ─── License ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProdSWLic {
    pub token: String,
}

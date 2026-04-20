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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseData {
    pub brands: Vec<Item>,
    #[serde(rename = "types")]
    pub product_types: Vec<Item>,
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
    #[serde(rename = "swVersion")]
    pub sw_version: String,
    #[serde(rename = "deviceName")]
    pub device_name: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecords {
    pub record_data: Vec<DeviceInfo>,
}

// ─── JSON-RPC Response ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: serde_json::Value,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnResult {
    pub serialno: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductIdResult {
    #[serde(rename = "productID")]
    pub product_id: String,
}

// ─── License ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProdSWLic {
    pub token: String,
}

// ─── App State ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub device_ip: String,
    pub current_product: Product,
    pub current_code_set: CodeSet,
}

use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::types::{BaseData, DeviceInfo, ExecutDataList, Product};

pub struct DataManager {
    data_dir: PathBuf,
}

impl DataManager {
    /// Creates a DataManager using the given app data directory.
    /// All config and runtime files are stored in the same directory.
    pub fn new(data_dir: PathBuf) -> Self {
        if !data_dir.exists() {
            let _ = fs::create_dir_all(&data_dir);
        }

        Self { data_dir }
    }

    fn read_json<T: DeserializeOwned>(&self, filename: &str) -> Result<T, String> {
        let path = self.data_dir.join(filename);
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", filename, e))
    }

    fn write_json<T: Serialize>(&self, filename: &str, data: &T) -> Result<(), String> {
        let path = self.data_dir.join(filename);
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize {}: {}", filename, e))?;

        fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))
    }

    pub fn load_base_data(&self) -> BaseData {
        match self.read_json("basecfgdata.json") {
            Ok(data) => data,
            Err(_) => {
                let data = Self::embedded_base_data();
                let _ = self.write_json("basecfgdata.json", &data);
                data
            }
        }
    }

    pub fn save_base_data(&self, data: &BaseData) -> Result<(), String> {
        self.write_json("basecfgdata.json", data)
    }

    fn embedded_base_data() -> BaseData {
        const JSON: &str = include_str!("../../portable/basecfgdata.json");
        serde_json::from_str(JSON).unwrap_or_default()
    }

    pub fn load_product_selection(&self, base_data: &BaseData) -> Product {
        self.read_json("selectdata.json").unwrap_or_else(|_| {
            let brand = base_data.brands.first().map(|b| b.name.clone()).unwrap_or_default();
            let product_type = base_data.product_types.first().map(|t| t.name.clone()).unwrap_or_default();
            let fac = base_data.factories.first().map(|f| f.name.clone()).unwrap_or_default();
            Product { brand, product_type, fac }
        })
    }

    pub fn save_product_selection(&self, product: &Product) -> Result<(), String> {
        self.write_json("selectdata.json", product)
    }

    pub fn load_execute_data(&self) -> Result<ExecutDataList, String> {
        self.read_json("execute_sn_data.json")
    }

    pub fn save_execute_data(&self, data: &ExecutDataList) -> Result<(), String> {
        self.write_json("execute_sn_data.json", data)
    }

    fn csv_escape(s: &str) -> String {
        if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }

    pub fn append_csv_record(&self, record: &DeviceInfo) -> Result<(), String> {
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let filename = format!("{}.csv", date);
        let path = self.data_dir.join(&filename);

        let need_header = !path.exists() || fs::metadata(&path).map(|m| m.len() == 0).unwrap_or(true);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Failed to open {}: {}", filename, e))?;

        if need_header {
            writeln!(file, "日期,IMEI,SN,软件版本,设备名称,激活状态")
                .map_err(|e| format!("Failed to write header for {}: {}", filename, e))?;
        }

        let status = if record.activated { "已激活" } else { "未激活" };

        writeln!(
            file,
            "{},{},{},{},{},{}",
            record.timestamp,
            Self::csv_escape(&record.imei),
            Self::csv_escape(&record.sn),
            Self::csv_escape(&record.sw_version),
            Self::csv_escape(&record.device_name),
            status,
        )
        .map_err(|e| format!("Failed to write record to {}: {}", filename, e))?;

        Ok(())
    }
}

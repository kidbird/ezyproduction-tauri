use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::types::{BaseData, DeviceRecords, ExecutDataList, Product};

pub struct DataManager {
    base_dir: PathBuf,
    data_dir: PathBuf,
}

impl DataManager {
    /// Creates a DataManager using the executable's directory (portable mode)
    pub fn portable(exe_dir: &Path) -> Self {
        let data_dir = exe_dir.join("Data");
        
        if !data_dir.exists() {
            let _ = fs::create_dir_all(&data_dir);
        }
        
        Self {
            base_dir: exe_dir.to_path_buf(),
            data_dir,
        }
    }

    pub fn read_json<T: DeserializeOwned>(&self, filename: &str) -> Result<T, String> {
        let path = self.data_dir.join(filename);
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;
        
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", filename, e))
    }

    pub fn write_json<T: Serialize>(&self, filename: &str, data: &T) -> Result<(), String> {
        let path = self.data_dir.join(filename);
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize {}: {}", filename, e))?;
        
        fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))
    }

    pub fn read_config_json<T: DeserializeOwned>(&self, filename: &str) -> Result<T, String> {
        let path = self.base_dir.join(filename);
        let content = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read {}: {}", filename, e))?;
        
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse {}: {}", filename, e))
    }

    pub fn write_config_json<T: Serialize>(&self, filename: &str, data: &T) -> Result<(), String> {
        let path = self.base_dir.join(filename);
        let content = serde_json::to_string_pretty(data)
            .map_err(|e| format!("Failed to serialize {}: {}", filename, e))?;
        
        fs::write(&path, content)
            .map_err(|e| format!("Failed to write {}: {}", filename, e))
    }

    pub fn load_base_data(&self) -> Result<BaseData, String> {
        self.read_config_json("basecfgdata.json")
    }

    pub fn load_product_selection(&self) -> Result<Product, String> {
        self.read_config_json("selectdata.json")
    }

    pub fn save_product_selection(&self, product: &Product) -> Result<(), String> {
        self.write_config_json("selectdata.json", product)
    }

    pub fn load_execute_data(&self) -> Result<ExecutDataList, String> {
        self.read_json("execute_sn_data.json")
    }

    pub fn save_execute_data(&self, data: &ExecutDataList) -> Result<(), String> {
        self.write_json("execute_sn_data.json", data)
    }

    pub fn load_device_records(&self) -> Result<DeviceRecords, String> {
        self.read_json("production_device_data.json")
    }

    pub fn save_device_records(&self, records: &DeviceRecords) -> Result<(), String> {
        self.write_json("production_device_data.json", records)
    }
}

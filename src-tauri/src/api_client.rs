use reqwest::Client;
use serde_json::json;
use std::sync::Mutex;

use crate::types::{DeviceInfo, JsonRpcResponse};

pub struct DeviceClient {
    client: Client,
    ip: Mutex<String>,
}

impl Clone for DeviceClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            ip: Mutex::new(self.ip.lock().unwrap().clone()),
        }
    }
}

impl DeviceClient {
    pub fn new(ip: &str) -> Self {
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            ip: Mutex::new(ip.to_string()),
        }
    }

    pub fn update_ip(&self, ip: &str) {
        if let Ok(mut current_ip) = self.ip.lock() {
            *current_ip = ip.to_string();
        }
    }

    fn base_url(&self) -> String {
        let ip = self.ip.lock().unwrap_or_else(|e| e.into_inner());
        format!("http://{}/arixoapi", ip)
    }

    async fn send_request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        let url = self.base_url();
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": "22.01"
        });

        let response = self.client
            .post(&url)
            .header("Content-Type", "text/plain")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        let json: JsonRpcResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(json.result)
    }

    pub async fn set_serial_no(&self, sn: &str) -> Result<bool, String> {
        let _ = self.send_request("SetSerialNo", json!({ "serialno": sn })).await?;
        
        // Verify by reading back
        let result = self.get_serial_no().await?;
        Ok(result == sn)
    }

    pub async fn get_serial_no(&self) -> Result<String, String> {
        let result = self.send_request("GetSerialNo", json!({})).await?;
        result.get("serialno")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to parse serial number".to_string())
    }

    pub async fn set_product_id(&self, product_id: &str) -> Result<bool, String> {
        let _ = self.send_request("SetProductID2", json!({ "productID": product_id })).await?;
        
        // Verify by reading back
        let result = self.get_product_id().await?;
        Ok(result == product_id)
    }

    pub async fn get_product_id(&self) -> Result<String, String> {
        let result = self.send_request("GetProductID2", json!({})).await?;
        result.get("productID")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "Failed to parse product ID".to_string())
    }

    pub async fn get_device_info(&self) -> Result<DeviceInfo, String> {
        let result = self.send_request("GetDeviceInfo", json!({})).await?;
        
        let device_info = DeviceInfo {
            imei: result.get("IMEI").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            iccid: result.get("ICCID").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            sn: result.get("sn").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            sw_version: result.get("SwVersion").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            device_name: result.get("DeviceName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        Ok(device_info)
    }

    pub async fn set_sim_slot(&self, slot: i32) -> Result<bool, String> {
        let _ = self.send_request("SetSimSlot", json!({ "slot": slot })).await?;
        Ok(true)
    }

    pub async fn validate_license(&self, lic_str: &str) -> Result<bool, String> {
        let result = self.send_request("ValidateLicense", json!({ "licStr": lic_str })).await?;
        
        // If result has no "code" field, activation succeeded
        Ok(result.get("code").is_none())
    }
}

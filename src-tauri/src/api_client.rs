use reqwest::Client;
use serde_json::json;
use std::sync::Mutex;

use crate::types::*;

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
        format!("http://{}/api", ip)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, action: &str) -> Result<T, String> {
        let url = format!("{}/{}", self.base_url(), action);
        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        response.json().await.map_err(|e| format!("Failed to parse response: {}", e))
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        action: &str,
        body: serde_json::Value,
    ) -> Result<T, String> {
        let url = format!("{}/{}", self.base_url(), action);
        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        response.json().await.map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn get_device_activate_info(&self) -> Result<DeviceActivateInfo, String> {
        let resp: ApiResponse<DeviceActivateInfo> = self.get("device_activate_info").await?;
        if resp.code != 200 {
            return Err(format!("API error: {}", resp.message));
        }
        Ok(resp.data)
    }

    pub async fn get_device_name(&self) -> Result<String, String> {
        let resp: ApiResponse<DeviceNameData> = self.get("device_name_get").await?;
        if resp.code != 200 {
            return Err(format!("API error: {}", resp.message));
        }
        Ok(resp.data.device_name)
    }

    pub async fn get_device_sn(&self) -> Result<String, String> {
        let resp: ApiResponse<SerialNumberData> = self.get("device_sn_get").await?;
        if resp.code != 200 {
            return Err(format!("API error: {}", resp.message));
        }
        Ok(resp.data.serial_number)
    }

    pub async fn set_device_sn(&self, sn: &str) -> Result<bool, String> {
        let body = json!({ "serial_number": sn });
        let resp: ApiResponse<EmptyData> = self.post("device_sn_set", body).await?;
        if resp.code != 200 {
            return Err(format!("API error: {}", resp.message));
        }

        // Verify by reading back
        let readback = self.get_device_sn().await?;
        Ok(readback == sn)
    }

    pub async fn activate_device(&self, lic_content: &str) -> Result<(), String> {
        let body = json!({ "lic_content": lic_content });
        let resp: ApiResponse<EmptyData> = self.post("device_activate", body).await?;
        if resp.code != 200 {
            return Err(format!("API error: {}", resp.message));
        }
        Ok(())
    }

    pub async fn get_license_status(&self) -> Result<ActivationStatus, String> {
        let resp: ApiResponse<ActivationStatus> = self.get("license_get").await?;
        if resp.code != 200 {
            return Err(format!("API error: {}", resp.message));
        }
        Ok(resp.data)
    }

    pub async fn get_device_info(&self) -> Result<DeviceInfo, String> {
        let activate_info = self.get_device_activate_info().await?;
        let device_name = self.get_device_name().await?;
        let sn = self.get_device_sn().await?;

        Ok(DeviceInfo {
            imei: activate_info.imei,
            iccid: activate_info.iccid,
            sn,
            sw_version: activate_info.firmware_version,
            device_name,
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    }
}

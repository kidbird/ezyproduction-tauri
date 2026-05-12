use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use mac_address;
use std::fs;

use crate::types::ProdSWLic;

const LICENSE_SECRET: &str = "PPRROODDUUCCTT123456";

#[derive(Debug, Serialize, Deserialize)]
struct LicenseClaims {
    mac: String,
    level: Option<String>,
    exp: Option<u64>,
}

pub fn read_license_file(base_dir: &std::path::Path) -> Result<ProdSWLic, String> {
    let path = base_dir.join("lic.dat");
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read license file: {}", e))?;
    
    let lic: ProdSWLic = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse license: {}", e))?;
    
    Ok(lic)
}

pub fn validate_license(token: &str) -> Result<bool, String> {
    let key = DecodingKey::from_secret(LICENSE_SECRET.as_bytes());
    let mut validation = Validation::new(Algorithm::HS512);
    validation.validate_exp = false; // Skip expiration check if not set
    
    let token_data = decode::<LicenseClaims>(token, &key, &validation)
        .map_err(|e| format!("Token validation failed: {}", e))?;
    
    let claims = token_data.claims;
    
    // Verify MAC address
    if !claims.mac.is_empty() {
        let current_mac = get_current_mac()?;
        if claims.mac != current_mac {
            return Err(format!(
                "MAC address mismatch: expected {}, got {}",
                claims.mac, current_mac
            ));
        }
    }
    
    Ok(true)
}

fn get_current_mac() -> Result<String, String> {
    // Get the first available MAC address
    match mac_address::get_mac_address() {
        Ok(Some(mac)) => {
            let bytes = mac.bytes();
            Ok(bytes.iter().map(|b| format!("{:02X}", b)).collect::<String>())
        }
        _ => Err("Failed to get MAC address".to_string()),
    }
}

// Generate device JWT for activation
pub fn generate_device_jwt(imei: &str, level: i32) -> Result<String, String> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use chrono::{Utc, Duration};
    
    #[derive(Serialize)]
    struct DeviceClaims {
        iat: i64,
        exp: i64,
        imei: String,
        level: i32,
    }
    
    let key = "AARRIIXXOO22001177";
    let now = Utc::now();
    let claims = DeviceClaims {
        iat: now.timestamp(),
        exp: (now + Duration::days(365)).timestamp(),
        imei: imei.to_string(),
        level,
    };
    
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(key.as_bytes()),
    )
    .map_err(|e| format!("Failed to generate JWT: {}", e))
}

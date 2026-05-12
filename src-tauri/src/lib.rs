use std::sync::Mutex;
use tauri::{State, Manager};

mod api_client;
mod data;
mod license;
mod sn_generator;
mod types;

use api_client::DeviceClient;
use data::DataManager;
use license::{generate_device_jwt, validate_license, read_license_file};
use sn_generator::{generate_sn, increment_seq, update_code_set};
use types::*;

pub struct AppState {
    pub device_client: Mutex<Option<DeviceClient>>,
    pub data_manager: Mutex<Option<DataManager>>,
    pub current_product: Mutex<Product>,
    pub current_code_set: Mutex<CodeSet>,
    pub device_records: Mutex<Option<DeviceRecords>>,
    pub execute_data: Mutex<Option<ExecutDataList>>,
    pub base_data: Mutex<Option<BaseData>>,
}

// ─── Initialization ─────────────────────────────────────────────────────────

#[tauri::command]
fn init_app(app_handle: tauri::AppHandle) -> Result<bool, String> {
    // Use executable directory for portable mode
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get exe path: {}", e))?;
    let exe_dir = exe_path.parent()
        .ok_or("Failed to get exe directory")?;
    
    let data_manager = DataManager::portable(exe_dir);
    
    // Load base data
    let base_data = data_manager.load_base_data().ok();
    
    // Load product selection
    let product = data_manager.load_product_selection().unwrap_or_else(|_| Product {
        brand: "Arixo".to_string(),
        product_type: "5G Router".to_string(),
        fac: "F1".to_string(),
    });
    
    // Load execute data
    let execute_data = data_manager.load_execute_data().ok();
    
    // Load device records
    let device_records = data_manager.load_device_records().ok();
    
    // Initialize code set
    let mut code_set = CodeSet {
        brand_code: "".to_string(),
        type_code: "".to_string(),
        fac_code: "".to_string(),
        year_code: "".to_string(),
        mon_code: "".to_string(),
        seq_code: "00001".to_string(),
    };
    
    // Resolve codes from base data
    if let Some(ref base) = base_data {
        for brand in &base.brands {
            if brand.name == product.brand {
                code_set.brand_code = brand.code.clone();
                break;
            }
        }
        for type_item in &base.product_types {
            if type_item.name == product.product_type {
                code_set.type_code = type_item.code.clone();
                break;
            }
        }
        for factory in &base.factories {
            if factory.name == product.fac {
                code_set.fac_code = factory.code.clone();
                break;
            }
        }
    }
    
    update_code_set(&mut code_set);
    
    // Restore sequence from execute data
    if let Some(ref ex_data) = execute_data {
        let prefix = format!("{}{}{}{}{}", 
            code_set.brand_code, code_set.type_code, code_set.fac_code,
            code_set.year_code, code_set.mon_code);
        
        for ex in &ex_data.exe_data_list {
            if ex.prefix_str == prefix {
                code_set.seq_code = increment_seq(&ex.curret_seq_no).unwrap_or("00001".to_string());
                break;
            }
        }
    }
    
    // Store state
    let state = app_handle.state::<AppState>();
    *state.data_manager.lock().unwrap() = Some(data_manager);
    *state.current_product.lock().unwrap() = product;
    *state.current_code_set.lock().unwrap() = code_set;
    *state.device_records.lock().unwrap() = device_records;
    *state.execute_data.lock().unwrap() = execute_data;
    *state.base_data.lock().unwrap() = base_data;
    
    Ok(true)
}

// ─── License ────────────────────────────────────────────────────────────────

#[tauri::command]
fn check_license() -> Result<bool, String> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .ok_or("Failed to get exe directory")?;
    let lic = read_license_file(&exe_dir)?;
    validate_license(&lic.token)
}

// ─── Product & Code Set ─────────────────────────────────────────────────────

#[tauri::command]
fn get_base_data(state: State<'_, AppState>) -> Result<BaseData, String> {
    let base = state.base_data.lock().unwrap();
    base.clone().ok_or_else(|| "Base data not loaded".to_string())
}

#[tauri::command]
fn get_current_product(state: State<'_, AppState>) -> Result<Product, String> {
    let product = state.current_product.lock().unwrap();
    Ok(product.clone())
}

#[tauri::command]
fn set_product(
    brand: String,
    product_type: String,
    fac: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let mut product = state.current_product.lock().unwrap();
    product.brand = brand.clone();
    product.product_type = product_type.clone();
    product.fac = fac.clone();
    
    // Update code set
    let mut code_set = state.current_code_set.lock().unwrap();
    let base = state.base_data.lock().unwrap();
    
    if let Some(ref base_data) = *base {
        for b in &base_data.brands {
            if b.name == brand {
                code_set.brand_code = b.code.clone();
                break;
            }
        }
        for t in &base_data.product_types {
            if t.name == product_type {
                code_set.type_code = t.code.clone();
                break;
            }
        }
        for f in &base_data.factories {
            if f.name == fac {
                code_set.fac_code = f.code.clone();
                break;
            }
        }
    }
    
    update_code_set(&mut code_set);
    
    // Save product selection
    if let Some(ref dm) = *state.data_manager.lock().unwrap() {
        dm.save_product_selection(&product)?;
    }
    
    Ok(generate_sn(&code_set))
}

#[tauri::command]
fn get_current_sn(state: State<'_, AppState>) -> Result<String, String> {
    let code_set = state.current_code_set.lock().unwrap();
    Ok(generate_sn(&code_set))
}

#[tauri::command]
fn get_code_set(state: State<'_, AppState>) -> Result<CodeSet, String> {
    let code_set = state.current_code_set.lock().unwrap();
    Ok(code_set.clone())
}

// ─── Device Communication ───────────────────────────────────────────────────

#[tauri::command]
fn set_device_ip(ip: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut client_opt = state.device_client.lock().unwrap();
    
    match client_opt.as_mut() {
        Some(client) => {
            client.update_ip(&ip);
        }
        None => {
            *client_opt = Some(DeviceClient::new(&ip));
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn write_sn_to_device(sn: String, state: State<'_, AppState>) -> Result<bool, String> {
    let client = {
        let guard = state.device_client.lock().unwrap();
        guard.as_ref().ok_or("Device client not initialized")?.clone()
    };
    client.set_device_sn(&sn).await
}

#[tauri::command]
async fn get_device_info_from_device(state: State<'_, AppState>) -> Result<DeviceInfo, String> {
    let client = {
        let guard = state.device_client.lock().unwrap();
        guard.as_ref().ok_or("Device client not initialized")?.clone()
    };
    client.get_device_info().await
}

#[tauri::command]
async fn activate_device(state: State<'_, AppState>) -> Result<bool, String> {
    let client = {
        let guard = state.device_client.lock().unwrap();
        guard.as_ref().ok_or("Device client not initialized")?.clone()
    };
    
    // Get device activate info (IMEI)
    let activate_info = client.get_device_activate_info().await?;
    
    // Generate JWT locally (HS256 + exp)
    let token = generate_device_jwt(&activate_info.imei, 1)?;
    
    // Write license to device
    client.activate_device(&token).await?;
    
    // Verify activation status
    let status = client.get_license_status().await?;
    Ok(status.valid)
}

// ─── Data Management ────────────────────────────────────────────────────────

#[tauri::command]
fn save_execute_data(state: State<'_, AppState>) -> Result<(), String> {
    let mut execute_data = state.execute_data.lock().unwrap();
    let code_set = state.current_code_set.lock().unwrap();
    let product = state.current_product.lock().unwrap();
    
    let prefix = format!("{}{}{}{}{}",
        code_set.brand_code, code_set.type_code, code_set.fac_code,
        code_set.year_code, code_set.mon_code);
    
    if execute_data.is_none() {
        *execute_data = Some(ExecutDataList {
            exe_data_list: Vec::new(),
        });
    }
    
    if let Some(ref mut ex_data) = *execute_data {
        let mut found = false;
        for ex in &mut ex_data.exe_data_list {
            if ex.prefix_str == prefix {
                ex.date_str = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                ex.curret_seq_no = code_set.seq_code.clone();
                ex.product_type = product.product_type.clone();
                found = true;
                break;
            }
        }
        
        if !found {
            ex_data.exe_data_list.push(ExecuteData {
                date_str: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                product_type: product.product_type.clone(),
                prefix_str: prefix,
                curret_seq_no: code_set.seq_code.clone(),
            });
        }
    }
    
    // Save to disk
    if let Some(ref dm) = *state.data_manager.lock().unwrap() {
        dm.save_execute_data(execute_data.as_ref().unwrap())?;
    }
    
    Ok(())
}

#[tauri::command]
fn save_device_record(device_info: DeviceInfo, state: State<'_, AppState>) -> Result<(), String> {
    let mut records = state.device_records.lock().unwrap();
    
    if records.is_none() {
        *records = Some(DeviceRecords {
            record_data: Vec::new(),
        });
    }
    
    if let Some(ref mut recs) = *records {
        let mut found = false;
        for rec in &mut recs.record_data {
            if rec.imei == device_info.imei {
                rec.iccid = device_info.iccid.clone();
                rec.sn = device_info.sn.clone();
                rec.sw_version = device_info.sw_version.clone();
                rec.device_name = device_info.device_name.clone();
                rec.timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                found = true;
                break;
            }
        }
        
        if !found {
            let mut new_rec = device_info.clone();
            new_rec.timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            recs.record_data.push(new_rec);
        }
    }
    
    // Save to disk
    if let Some(ref dm) = *state.data_manager.lock().unwrap() {
        dm.save_device_records(records.as_ref().unwrap())?;
    }
    
    Ok(())
}

#[tauri::command]
fn increment_sequence(state: State<'_, AppState>) -> Result<String, String> {
    let mut code_set = state.current_code_set.lock().unwrap();
    code_set.seq_code = increment_seq(&code_set.seq_code)?;
    Ok(generate_sn(&code_set))
}

// ─── Tauri App Setup ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            device_client: Mutex::new(None),
            data_manager: Mutex::new(None),
            current_product: Mutex::new(Product {
                brand: "".to_string(),
                product_type: "".to_string(),
                fac: "".to_string(),
            }),
            current_code_set: Mutex::new(CodeSet {
                brand_code: "".to_string(),
                type_code: "".to_string(),
                fac_code: "".to_string(),
                year_code: "".to_string(),
                mon_code: "".to_string(),
                seq_code: "00001".to_string(),
            }),
            device_records: Mutex::new(None),
            execute_data: Mutex::new(None),
            base_data: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            init_app,
            check_license,
            get_base_data,
            get_current_product,
            set_product,
            get_current_sn,
            get_code_set,
            set_device_ip,
            write_sn_to_device,
            get_device_info_from_device,
            activate_device,
            save_execute_data,
            save_device_record,
            increment_sequence,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

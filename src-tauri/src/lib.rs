use std::sync::Mutex;
use tauri::{Manager, State};

mod api_client;
mod data;
mod license;
mod sn_generator;
mod types;

use api_client::DeviceClient;
use data::DataManager;
use license::{generate_device_jwt, read_license_file, validate_license};
use sn_generator::{generate_sn, increment_seq, update_code_set};
use types::*;

pub struct AppState {
    pub device_client: Mutex<Option<DeviceClient>>,
    pub data_manager: Mutex<Option<DataManager>>,
    pub current_product: Mutex<Product>,
    pub current_code_set: Mutex<CodeSet>,
    pub device_records: Mutex<Option<DeviceRecords>>,
    pub execute_data: Mutex<Option<ExecutDataList>>,
    pub base_data: Mutex<BaseData>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn resolve_codes(base_data: &BaseData, product: &Product, code_set: &mut CodeSet) {
    for brand in &base_data.brands {
        if brand.name == product.brand {
            code_set.brand_code = brand.code.clone();
            break;
        }
    }
    for type_item in &base_data.product_types {
        if type_item.name == product.product_type {
            code_set.type_code = type_item.code.clone();
            break;
        }
    }
    for factory in &base_data.factories {
        if factory.name == product.fac {
            code_set.fac_code = factory.code.clone();
            break;
        }
    }
}

fn next_index(items: &[Item]) -> i32 {
    items.iter().map(|i| i.index).max().unwrap_or(0) + 1
}

fn save_base_data_and_notify(state: &State<'_, AppState>) -> Result<(), String> {
    let dm = state.data_manager.lock().unwrap();
    let dm = dm.as_ref().ok_or("Data manager not initialized")?;
    let base = state.base_data.lock().unwrap();
    dm.save_base_data(&base)
}

// ─── Initialization ─────────────────────────────────────────────────────────

#[tauri::command]
fn init_app(app_handle: tauri::AppHandle) -> Result<bool, String> {
    let data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    let data_manager = DataManager::new(data_dir);

    let base_data = data_manager.load_base_data();
    let product = data_manager.load_product_selection(&base_data);
    let execute_data = data_manager.load_execute_data().ok();
    let device_records = data_manager.load_device_records().ok();

    let mut code_set = CodeSet {
        brand_code: String::new(),
        type_code: String::new(),
        fac_code: String::new(),
        year_code: String::new(),
        mon_code: String::new(),
        seq_code: "00001".to_string(),
    };

    resolve_codes(&base_data, &product, &mut code_set);
    update_code_set(&mut code_set);

    if let Some(ref ex_data) = execute_data {
        let prefix = format!(
            "{}{}{}{}{}",
            code_set.brand_code,
            code_set.type_code,
            code_set.fac_code,
            code_set.year_code,
            code_set.mon_code
        );
        for ex in &ex_data.exe_data_list {
            if ex.prefix_str == prefix {
                code_set.seq_code = increment_seq(&ex.curret_seq_no).unwrap_or("00001".to_string());
                break;
            }
        }
    }

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
    Ok(state.base_data.lock().unwrap().clone())
}

#[tauri::command]
fn get_current_product(state: State<'_, AppState>) -> Result<Product, String> {
    Ok(state.current_product.lock().unwrap().clone())
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

    let mut code_set = state.current_code_set.lock().unwrap();
    let base = state.base_data.lock().unwrap();

    resolve_codes(&base, &product, &mut code_set);
    update_code_set(&mut code_set);

    if let Some(ref dm) = *state.data_manager.lock().unwrap() {
        dm.save_product_selection(&product)?;
    }

    Ok(generate_sn(&code_set))
}

#[tauri::command]
fn get_current_sn(state: State<'_, AppState>) -> Result<String, String> {
    Ok(generate_sn(&state.current_code_set.lock().unwrap()))
}

#[tauri::command]
fn get_code_set(state: State<'_, AppState>) -> Result<CodeSet, String> {
    Ok(state.current_code_set.lock().unwrap().clone())
}

// ─── Base Data Management (Brand / Type / Factory CRUD) ─────────────────────

macro_rules! crud_commands {
    ($add_name:ident, $remove_name:ident, $field:ident, $label:literal) => {
        #[tauri::command]
        fn $add_name(name: String, code: String, state: State<'_, AppState>) -> Result<BaseData, String> {
            if name.trim().is_empty() || code.trim().is_empty() {
                return Err(format!("{}名称和编码不能为空", $label));
            }
            {
                let mut base = state.base_data.lock().unwrap();
                if base.$field.iter().any(|i| i.name == name) {
                    return Err(format!("{}名称已存在: {}", $label, name));
                }
                if base.$field.iter().any(|i| i.code == code) {
                    return Err(format!("{}编码已存在: {}", $label, code));
                }
                let idx = next_index(&base.$field);
                base.$field.push(Item { name: name.trim().to_string(), index: idx, code: code.trim().to_string() });
            }
            save_base_data_and_notify(&state)?;
            Ok(state.base_data.lock().unwrap().clone())
        }

        #[tauri::command]
        fn $remove_name(name: String, state: State<'_, AppState>) -> Result<BaseData, String> {
            {
                let mut base = state.base_data.lock().unwrap();
                let before = base.$field.len();
                base.$field.retain(|i| i.name != name);
                if base.$field.len() == before {
                    return Err(format!("{}不存在: {}", $label, name));
                }
            }
            save_base_data_and_notify(&state)?;
            Ok(state.base_data.lock().unwrap().clone())
        }
    };
}

crud_commands!(add_brand, remove_brand, brands, "品牌");
crud_commands!(add_product_type, remove_product_type, product_types, "产品类型");
crud_commands!(add_factory, remove_factory, factories, "工厂");

// ─── Device Communication ───────────────────────────────────────────────────

#[tauri::command]
fn set_device_ip(ip: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut client_opt = state.device_client.lock().unwrap();
    match client_opt.as_mut() {
        Some(client) => client.update_ip(&ip),
        None => *client_opt = Some(DeviceClient::new(&ip)),
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
    let activate_info = client.get_device_activate_info().await?;
    let token = generate_device_jwt(&activate_info.imei, 1)?;
    client.activate_device(&token).await?;
    let status = client.get_license_status().await?;
    Ok(status.valid)
}

// ─── Data Management ────────────────────────────────────────────────────────

#[tauri::command]
fn save_execute_data(state: State<'_, AppState>) -> Result<(), String> {
    let mut execute_data = state.execute_data.lock().unwrap();
    let code_set = state.current_code_set.lock().unwrap();
    let product = state.current_product.lock().unwrap();

    let prefix = format!(
        "{}{}{}{}{}",
        code_set.brand_code, code_set.type_code, code_set.fac_code,
        code_set.year_code, code_set.mon_code
    );

    if execute_data.is_none() {
        *execute_data = Some(ExecutDataList { exe_data_list: Vec::new() });
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

    if let Some(ref dm) = *state.data_manager.lock().unwrap() {
        dm.save_execute_data(execute_data.as_ref().unwrap())?;
    }

    Ok(())
}

#[tauri::command]
fn save_device_record(device_info: DeviceInfo, state: State<'_, AppState>) -> Result<(), String> {
    let mut records = state.device_records.lock().unwrap();

    if records.is_none() {
        *records = Some(DeviceRecords { record_data: Vec::new() });
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
                brand: String::new(),
                product_type: String::new(),
                fac: String::new(),
            }),
            current_code_set: Mutex::new(CodeSet {
                brand_code: String::new(),
                type_code: String::new(),
                fac_code: String::new(),
                year_code: String::new(),
                mon_code: String::new(),
                seq_code: "00001".to_string(),
            }),
            device_records: Mutex::new(None),
            execute_data: Mutex::new(None),
            base_data: Mutex::new(BaseData::default()),
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
            add_brand,
            remove_brand,
            add_product_type,
            remove_product_type,
            add_factory,
            remove_factory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

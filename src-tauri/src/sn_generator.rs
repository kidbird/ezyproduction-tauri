use chrono::{Local, Datelike};

use crate::types::CodeSet;

pub fn get_year_code() -> String {
    let year = Local::now().format("%Y").to_string();
    year.chars().last().unwrap_or('0').to_string()
}

pub fn get_mon_code() -> String {
    let month = Local::now().month() as u32;
    format!("{:X}", month).to_uppercase()
}

pub fn increment_seq(seq: &str) -> Result<String, String> {
    let number: u32 = seq.parse().map_err(|_| "Invalid sequence number")?;
    let incremented = number + 1;
    
    if incremented > 99999 {
        return Err("Sequence exceeded maximum (99999)".to_string());
    }
    
    Ok(format!("{:05}", incremented))
}

pub fn generate_sn(code_set: &CodeSet) -> String {
    format!(
        "{}{}{}{}{}{}",
        code_set.brand_code,
        code_set.type_code,
        code_set.fac_code,
        code_set.year_code,
        code_set.mon_code,
        code_set.seq_code
    )
}

pub fn update_code_set(code_set: &mut CodeSet) {
    code_set.year_code = get_year_code();
    code_set.mon_code = get_mon_code();
}

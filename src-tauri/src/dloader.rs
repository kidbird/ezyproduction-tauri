//! Firmware download integration: drives the 32-bit `r26-cli` sidecar.
//!
//! FactoryTool stays 64-bit and never links the Unisoc DLLs. It spawns the
//! self-contained 32-bit `r26-cli.exe` sidecar, decoding its line-delimited
//! JSON into Tauri `firmware-event`s for the UI.

use serde::{Deserialize, Serialize};

// ── DTOs mirroring r26-core's serialized safety types ───────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRiskDto {
    pub file_id: String,
    pub file_type: String,
    /// Serialized RiskLevel variant name, e.g. "NvWrite", "Erase", "RfCalibration".
    pub risk_level: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReportDto {
    pub total_files: usize,
    pub safe_files: Vec<String>,
    pub nv_files: Vec<FileRiskDto>,
    pub rf_cali_files: Vec<FileRiskDto>,
    pub erase_files: Vec<FileRiskDto>,
    pub phasecheck_files: Vec<FileRiskDto>,
    pub has_risks: bool,
    pub touches_rf_calibration: bool,
    pub summary: String,
}

/// The decision for how to flash a given PAC.
#[derive(Debug, Clone, PartialEq)]
pub struct FlashPlan {
    /// Some(reason) => do NOT flash; show this message. None => proceed.
    pub blocked_reason: Option<String>,
    /// `--allow-*` flags to pass to `r26-cli download`.
    pub allow_flags: Vec<&'static str>,
}

/// Factory-safe flashing policy.
///
/// erase + NV writes are auto-allowed (a full firmware flash needs them, and
/// DLFrame.dll preserves the device's real calibration via Research-mode NV
/// backup). RF calibration and PhaseCheck are NEVER allowed — such PACs are
/// blocked outright so the sidecar is never even spawned for them.
pub fn plan_flash(report: &SafetyReportDto) -> FlashPlan {
    if report.touches_rf_calibration || !report.rf_cali_files.is_empty() {
        return FlashPlan {
            blocked_reason: Some("此 PAC 含射频校准分区，出于保护已禁止刷写".to_string()),
            allow_flags: vec![],
        };
    }
    if !report.phasecheck_files.is_empty() {
        return FlashPlan {
            blocked_reason: Some("此 PAC 含 PhaseCheck 生产数据分区，已禁止刷写".to_string()),
            allow_flags: vec![],
        };
    }
    let mut allow_flags = Vec::new();
    if !report.erase_files.is_empty() {
        allow_flags.push("--allow-erase");
    }
    if !report.nv_files.is_empty() {
        allow_flags.push("--allow-nv-write");
    }
    FlashPlan { blocked_reason: None, allow_flags }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risk(id: &str) -> FileRiskDto {
        FileRiskDto { file_id: id.into(), file_type: "".into(), risk_level: "".into(), reason: "".into() }
    }
    fn report() -> SafetyReportDto {
        SafetyReportDto {
            total_files: 0, safe_files: vec![], nv_files: vec![], rf_cali_files: vec![],
            erase_files: vec![], phasecheck_files: vec![], has_risks: false,
            touches_rf_calibration: false, summary: String::new(),
        }
    }

    #[test]
    fn safe_pac_proceeds_with_no_flags() {
        let plan = plan_flash(&report());
        assert_eq!(plan.blocked_reason, None);
        assert!(plan.allow_flags.is_empty());
    }

    #[test]
    fn erase_and_nv_auto_allowed() {
        let mut r = report();
        r.erase_files = vec![risk("EraseFlash")];
        r.nv_files = vec![risk("NV_NORFLASH")];
        let plan = plan_flash(&r);
        assert_eq!(plan.blocked_reason, None);
        assert_eq!(plan.allow_flags, vec!["--allow-erase", "--allow-nv-write"]);
    }

    #[test]
    fn rf_calibration_is_blocked() {
        let mut r = report();
        r.touches_rf_calibration = true;
        r.rf_cali_files = vec![risk("LTE_CALI")];
        let plan = plan_flash(&r);
        assert!(plan.blocked_reason.is_some());
        assert!(plan.allow_flags.is_empty());
    }

    #[test]
    fn phasecheck_is_blocked() {
        let mut r = report();
        r.phasecheck_files = vec![risk("PhaseCheck")];
        let plan = plan_flash(&r);
        assert!(plan.blocked_reason.is_some());
    }
}

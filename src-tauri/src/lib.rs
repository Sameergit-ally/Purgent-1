use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use purgent_core::modules::config::WipeStandard;
use purgent_core::modules::drive_eraser::{WipeRequest, WipeTarget};
use purgent_core::modules::file_eraser::{EraseRequest, EraseTarget, FileEraseStandard};
use purgent_core::modules::operations::{
    run_carve_with_progress, run_erase_with_progress, run_wipe_with_progress,
};
use purgent_core::modules::persistence::CaseDb;
use purgent_core::modules::reporting::{verify_report, Report};
use purgent_core::modules::signing::{identity_fingerprint, load_or_create_signing_key};
use purgent_core::modules::sync::{AuditSyncClient, SyncConfig};

pub struct AppState {
    signing_key: Vec<u8>,
    report_dir: PathBuf,
    case_db_path: PathBuf,
    operator_id: RwLock<String>,
    sync_config: Option<SyncConfig>,
    sync_enabled: RwLock<bool>,
}

impl AppState {
    fn open_db(&self) -> Result<CaseDb, String> {
        CaseDb::open(&self.case_db_path).map_err(|e| e.to_string())
    }
}

#[derive(Serialize)]
struct RepoIdentity {
    operator_id: String,
    fingerprint: String,
    report_dir: String,
    case_db: String,
}

#[derive(Serialize)]
struct WipeStandardInfo {
    id: String,
    label: &'static str,
}

#[derive(Serialize)]
struct EraseStandardInfo {
    id: String,
    label: &'static str,
}

#[derive(Serialize)]
struct ReportMeta {
    filename: String,
    report_id: String,
    operation_type: String,
    operator_id: String,
    target: String,
    start_time: String,
    standard_label: String,
    verified: bool,
    has_pdf: bool,
    capacity_bytes: u64,
}

#[derive(Serialize)]
struct ReportView {
    report: Report,
    verified: bool,
}

#[derive(Serialize, Clone)]
struct OperationOutcome {
    operation_id: String,
    report_json: PathBuf,
    report_pdf: PathBuf,
}

#[derive(Serialize)]
struct SyncStatus {
    enabled: bool,
    configured: bool,
    url: String,
    synced: u64,
    total: u64,
}

#[tauri::command]
fn hello(name: String) -> String {
    purgent_core::hello(&name)
}

#[tauri::command]
fn list_devices() -> Vec<purgent_core::modules::storage::Device> {
    purgent_core::modules::storage::list_devices()
}

#[tauri::command]
fn get_identity(state: State<'_, AppState>) -> RepoIdentity {
    RepoIdentity {
        operator_id: state.operator_id.read().unwrap().clone(),
        fingerprint: identity_fingerprint(&state.signing_key),
        report_dir: state.report_dir.display().to_string(),
        case_db: state.case_db_path.display().to_string(),
    }
}

#[tauri::command]
fn set_operator(state: State<'_, AppState>, operator_id: String) -> String {
    let id = operator_id.trim().to_string();
    if id.is_empty() {
        return state.operator_id.read().unwrap().clone();
    }
    *state.operator_id.write().unwrap() = id.clone();
    let path = config_dir().join("operator.txt");
    if std::fs::write(&path, &id).is_ok() {
        purgent_core::modules::log::log("selected", &format!("operator identity set to '{id}'"));
    }
    id
}

#[tauri::command]
fn get_wipe_standards() -> Vec<WipeStandardInfo> {
    [
        WipeStandard::Nist800_88Clear,
        WipeStandard::Nist800_88Purge,
        WipeStandard::Dod522022M,
    ]
    .into_iter()
    .map(|s| WipeStandardInfo {
        id: wipe_standard_id(s).to_string(),
        label: s.label(),
    })
    .collect()
}

#[tauri::command]
fn get_erase_standards() -> Vec<EraseStandardInfo> {
    [FileEraseStandard::NistClear, FileEraseStandard::Purge3Pass]
        .into_iter()
        .map(|s| EraseStandardInfo {
            id: erase_standard_id(s).to_string(),
            label: s.label(),
        })
        .collect()
}

#[tauri::command]
fn wipe_image(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    standard_id: String,
    confirmed: String,
) -> Result<String, String> {
    let target_path = PathBuf::from(&path);
    if !target_path.is_file() {
        return Err(format!("target is not a readable file: {path}"));
    }
    let standard = parse_wipe_standard(&standard_id)?;
    let target = WipeTarget::ImageFile(target_path);
    let request = WipeRequest {
        operator_id: state.operator_id.read().unwrap().clone(),
        target,
        standard,
        operator_confirmed_target: confirmed,
        block_size: None,
        fallback_acknowledged: false,
    };
    let outcome_code = spawn_wipe(app, state, request);
    Ok(outcome_code)
}

#[tauri::command]
fn wipe_device(
    app: AppHandle,
    state: State<'_, AppState>,
    device_path: String,
    capacity_bytes: u64,
    media_type_id: String,
    standard_id: String,
    fallback_acknowledged: bool,
    confirmed: String,
) -> Result<String, String> {
    if !looks_like_block_device(&device_path) {
        return Err("refusing to wipe anything but a block device path".into());
    }
    let standard = parse_wipe_standard(&standard_id)?;
    let media_type = purgent_core::modules::storage::parse_media_type(&media_type_id)?;
    let target = WipeTarget::Device {
        path: device_path.clone(),
        capacity_bytes,
        media_type,
    };
    let request = WipeRequest {
        operator_id: state.operator_id.read().unwrap().clone(),
        target,
        standard,
        operator_confirmed_target: confirmed,
        block_size: None,
        fallback_acknowledged,
    };
    let code = spawn_wipe(app, state, request);
    Ok(code)
}

fn looks_like_block_device(path: &str) -> bool {
    path.starts_with("\\\\.\\PHYSICALDRIVE")
        || path.starts_with("/dev/sd")
        || path.starts_with("/dev/vd")
        || path.starts_with("/dev/hd")
        || path.starts_with("/dev/nvme")
        || path.starts_with("/dev/mmcblk")
        || path.starts_with("/dev/sr")
        || path.starts_with("/dev/loop")
}

#[tauri::command]
fn erase_path(
    app: AppHandle,
    state: State<'_, AppState>,
    target_type: String,
    path: String,
    standard_id: String,
    confirmed: String,
) -> Result<String, String> {
    let target_path = PathBuf::from(&path);
    if !target_path.exists() {
        return Err(format!("target does not exist: {path}"));
    }
    let standard = parse_erase_standard(&standard_id)?;
    let target = match target_type.as_str() {
        "file" => EraseTarget::SingleFile(target_path),
        "folder" => EraseTarget::Folder(target_path),
        _ => return Err(format!("unknown target type '{target_type}'")),
    };
    let request = EraseRequest {
        operator_id: state.operator_id.read().unwrap().clone(),
        target,
        standard,
        operator_confirmed_target: confirmed,
        block_size: None,
    };
    let code = spawn_erase(app, state, request);
    Ok(code)
}

#[tauri::command]
fn carve_source(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    output_dir: String,
) -> Result<String, String> {
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    let code = spawn_carve(app, state, source, output_dir);
    Ok(code)
}

#[tauri::command]
fn list_reports(state: State<'_, AppState>) -> Result<Vec<ReportMeta>, String> {
    let db = state.open_db()?;
    let rows = db.all_report_meta().map_err(|e| e.to_string())?;
    let mut metas = Vec::new();
    for row in rows {
        let stored = db.report_json(&row.report_id).map_err(|e| e.to_string())?;
        let verified = verify_report(stored.as_bytes(), &state.signing_key);
        let pdf_ok = std::fs::metadata(&PathBuf::from(&row.pdf_path)).is_ok();
        let filename = Path::new(&row.json_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        metas.push(ReportMeta {
            filename,
            report_id: row.report_id,
            operation_type: row.operation_type,
            operator_id: row.operator_id,
            target: row.target,
            start_time: row.start_time,
            standard_label: row.standard_label,
            verified,
            has_pdf: pdf_ok,
            capacity_bytes: row.capacity_bytes,
        });
    }
    Ok(metas)
}

#[tauri::command]
fn read_report(state: State<'_, AppState>, filename: String) -> Result<ReportView, String> {
    let report_id = report_id_from_filename(&filename)?;
    let db = state.open_db()?;
    let stored = db.report_json(&report_id).map_err(|e| e.to_string())?;
    let report: Report = serde_json::from_str(&stored).map_err(|e| e.to_string())?;
    let verified = verify_report(stored.as_bytes(), &state.signing_key);
    Ok(ReportView { report, verified })
}

#[tauri::command]
fn open_report_pdf(state: State<'_, AppState>, filename: String) -> Result<(), String> {
    let report_id = report_id_from_filename(&filename)?;
    let db = state.open_db()?;
    let (_json, pdf) = db.report_paths(&report_id).map_err(|e| e.to_string())?;
    if !Path::new(&pdf).is_file() {
        return Err(format!("pdf report not found for {filename}"));
    }
    open_with_default_app(&pdf);
    Ok(())
}

#[tauri::command]
fn export_report_xml(state: State<'_, AppState>, filename: String) -> Result<String, String> {
    let report_id = report_id_from_filename(&filename)?;
    let db = state.open_db()?;
    let stored = db.report_json(&report_id).map_err(|e| e.to_string())?;
    let report: Report = serde_json::from_str(&stored).map_err(|e| e.to_string())?;
    let xml = purgent_core::modules::reporting::export_xml(&report);
    let xml_path = purgent_core::modules::reporting::save_report_xml(&report, &state.report_dir)
        .map_err(|e| e.to_string())?;
    let _ = xml;
    Ok(xml_path.display().to_string())
}

#[tauri::command]
fn open_report_xml(state: State<'_, AppState>, filename: String) -> Result<(), String> {
    let report_id = report_id_from_filename(&filename)?;
    let db = state.open_db()?;
    let stored = db.report_json(&report_id).map_err(|e| e.to_string())?;
    let report: Report = serde_json::from_str(&stored).map_err(|e| e.to_string())?;
    let xml_path = purgent_core::modules::reporting::save_report_xml(&report, &state.report_dir)
        .map_err(|e| e.to_string())?;
    open_with_default_app(&xml_path);
    Ok(())
}

#[tauri::command]
fn get_sync_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let enabled = *state.sync_enabled.read().unwrap();
    let configured = state.sync_config.is_some();
    let url = state
        .sync_config
        .as_ref()
        .map(|c| c.url.clone())
        .unwrap_or_default();
    let (synced, total) = state.open_db()?.sync_counts().map_err(|e| e.to_string())?;
    Ok(SyncStatus {
        enabled,
        configured,
        url,
        synced,
        total,
    })
}

#[tauri::command]
fn set_sync_enabled(state: State<'_, AppState>, enabled: bool) -> Result<SyncStatus, String> {
    if enabled && state.sync_config.is_none() {
        return Err(
            "cloud sync is not configured; set PURGENT_SUPABASE_URL and PURGENT_SUPABASE_ANON_KEY \
             environment variables and restart the app"
                .into(),
        );
    }
    *state.sync_enabled.write().unwrap() = enabled;
    let path = config_dir().join("sync_optin.txt");
    let _ = std::fs::write(&path, if enabled { "enabled" } else { "disabled" });
    purgent_core::modules::log::log(
        "reported",
        &format!(
            "cloud sync {}",
            if enabled { "enabled" } else { "disabled" }
        ),
    );
    get_sync_status(state)
}

#[tauri::command]
fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Result<SyncStatus, String> {
    if !*state.sync_enabled.read().unwrap() {
        return Err("cloud sync is disabled; enable it first".into());
    }
    let Some(cfg) = state.sync_config.clone() else {
        return Err("cloud sync is not configured".into());
    };
    let client = AuditSyncClient::new(cfg);
    let case_db_path = state.case_db_path.clone();
    std::thread::spawn(move || {
        if let Ok(db) = CaseDb::open(&case_db_path) {
            match client.sync_unpushed(&db) {
                Ok((pushed, failed)) => purgent_core::modules::log::log(
                    "reported",
                    &format!("cloud sync pushed={pushed} failed={}", failed.len()),
                ),
                Err(e) => purgent_core::modules::log::log(
                    "reported",
                    &format!("cloud sync unavailable: {e}"),
                ),
            }
        }
        let _ = app.emit("sync-state", ());
    });
    get_sync_status(state)
}

fn report_id_from_filename(filename: &str) -> Result<String, String> {
    let id = filename
        .strip_prefix("report-")
        .and_then(|s| s.strip_suffix(".json"))
        .or_else(|| {
            filename
                .strip_prefix("report-")
                .and_then(|s| s.strip_suffix(".pdf"))
        })
        .ok_or_else(|| format!("unrecognized report filename {filename}"))?;
    Ok(id.to_string())
}

#[cfg(windows)]
fn open_with_default_app(path: &Path) {
    let _ = std::process::Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("")
        .arg(path)
        .spawn();
}

#[cfg(not(windows))]
fn open_with_default_app(path: &Path) {
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn wipe_standard_id(s: WipeStandard) -> &'static str {
    match s {
        WipeStandard::Nist800_88Clear => "nist_800_88_clear",
        WipeStandard::Nist800_88Purge => "nist_800_88_purge",
        WipeStandard::Dod522022M => "dod_522022_m",
        WipeStandard::Ieee2883Purge => "ieee_2883_purge",
        WipeStandard::Iso27037Capture => "iso_27037_capture",
        WipeStandard::AtaSecureErase => "ata_secure_erase",
        WipeStandard::NvmeSanitize => "nvme_sanitize",
    }
}

fn erase_standard_id(s: FileEraseStandard) -> &'static str {
    match s {
        FileEraseStandard::NistClear => "nist_clear",
        FileEraseStandard::Purge3Pass => "purge_3_pass",
    }
}

fn parse_wipe_standard(id: &str) -> Result<WipeStandard, String> {
    match id {
        "nist_800_88_clear" => Ok(WipeStandard::Nist800_88Clear),
        "nist_800_88_purge" => Ok(WipeStandard::Nist800_88Purge),
        "dod_522022_m" => Ok(WipeStandard::Dod522022M),
        "ieee_2883_purge" => Ok(WipeStandard::Ieee2883Purge),
        "iso_27037_capture" => Ok(WipeStandard::Iso27037Capture),
        "ata_secure_erase" => Ok(WipeStandard::AtaSecureErase),
        "nvme_sanitize" => Ok(WipeStandard::NvmeSanitize),
        _ => Err(format!("unknown wipe standard '{id}'")),
    }
}

fn parse_erase_standard(id: &str) -> Result<FileEraseStandard, String> {
    match id {
        "nist_clear" => Ok(FileEraseStandard::NistClear),
        "purge_3_pass" => Ok(FileEraseStandard::Purge3Pass),
        _ => Err(format!("unknown erase standard '{id}'")),
    }
}

fn span_operation(app: &AppHandle, operation_type: &str) -> String {
    let operation_id = uuid::Uuid::new_v4().to_string();
    let _ = app.emit(
        "progress",
        purgent_core::modules::progress::ProgressUpdate {
            operation_id: operation_id.clone(),
            operation_type: operation_type.to_string(),
            phase: "queued".to_string(),
            bytes_done: 0,
            total_bytes: 0,
            message: "operation queued".to_string(),
        },
    );
    operation_id
}

fn spawn_wipe(app: AppHandle, state: State<'_, AppState>, request: WipeRequest) -> String {
    let signing_key = state.signing_key.clone();
    let report_dir = state.report_dir.clone();
    let case_db_path = state.case_db_path.clone();
    let sync_config = state.sync_config.clone();
    let sync_enabled = *state.sync_enabled.read().unwrap();
    let operation_id = span_operation(&app, "secure_erase");
    let thread_id = operation_id.clone();
    std::thread::spawn(move || {
        let mut progress: purgent_core::modules::progress::ProgressFn = &mut |update| {
            let _ = app.emit("progress", update);
        };
        let outcome = run_wipe_with_progress(request, &report_dir, &signing_key, &mut progress);
        match outcome {
            Ok(outcome) => {
                record_outcome(
                    &case_db_path,
                    &outcome.report,
                    &outcome.report_json_path,
                    &outcome.report_pdf_path,
                );
                attempt_sync(&case_db_path, sync_config, sync_enabled);
                finish_operation_ok(
                    &app,
                    thread_id,
                    outcome.report_json_path,
                    outcome.report_pdf_path,
                );
            }
            Err(err) => finish_operation_err(&app, thread_id, err),
        }
    });
    operation_id
}

fn spawn_erase(app: AppHandle, state: State<'_, AppState>, request: EraseRequest) -> String {
    let signing_key = state.signing_key.clone();
    let report_dir = state.report_dir.clone();
    let case_db_path = state.case_db_path.clone();
    let sync_config = state.sync_config.clone();
    let sync_enabled = *state.sync_enabled.read().unwrap();
    let operation_id = span_operation(&app, "file_erase");
    let thread_id = operation_id.clone();
    std::thread::spawn(move || {
        let mut progress: purgent_core::modules::progress::ProgressFn = &mut |update| {
            let _ = app.emit("progress", update);
        };
        let outcome = run_erase_with_progress(request, &report_dir, &signing_key, &mut progress);
        match outcome {
            Ok(outcome) => {
                record_outcome(
                    &case_db_path,
                    &outcome.report,
                    &outcome.report_json_path,
                    &outcome.report_pdf_path,
                );
                attempt_sync(&case_db_path, sync_config, sync_enabled);
                finish_operation_ok(
                    &app,
                    thread_id,
                    outcome.report_json_path,
                    outcome.report_pdf_path,
                );
            }
            Err(err) => finish_operation_err(&app, thread_id, err),
        }
    });
    operation_id
}

fn spawn_carve(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    output_dir: String,
) -> String {
    let signing_key = state.signing_key.clone();
    let report_dir = state.report_dir.clone();
    let case_db_path = state.case_db_path.clone();
    let sync_config = state.sync_config.clone();
    let sync_enabled = *state.sync_enabled.read().unwrap();
    let operation_id = span_operation(&app, "file_recovery");
    let thread_id = operation_id.clone();
    std::thread::spawn(move || {
        let mut progress: purgent_core::modules::progress::ProgressFn = &mut |update| {
            let _ = app.emit("progress", update);
        };
        let outcome = run_carve_with_progress(
            Path::new(&source),
            Path::new(&output_dir),
            &report_dir,
            &signing_key,
            &mut progress,
        );
        match outcome {
            Ok(outcome) => {
                record_outcome(
                    &case_db_path,
                    &outcome.report,
                    &outcome.report_json_path,
                    &outcome.report_pdf_path,
                );
                attempt_sync(&case_db_path, sync_config, sync_enabled);
                finish_operation_ok(
                    &app,
                    thread_id,
                    outcome.report_json_path,
                    outcome.report_pdf_path,
                );
            }
            Err(err) => finish_operation_err(&app, thread_id, err),
        }
    });
    operation_id
}

fn attempt_sync(case_db_path: &Path, sync_config: Option<SyncConfig>, sync_enabled: bool) {
    if !sync_enabled {
        return;
    }
    let Some(cfg) = sync_config else {
        return;
    };
    let client = AuditSyncClient::new(cfg);
    let case_db_path = case_db_path.to_path_buf();
    std::thread::spawn(move || {
        if let Ok(db) = CaseDb::open(&case_db_path) {
            match client.sync_unpushed(&db) {
                Ok((pushed, failed)) => {
                    if failed.is_empty() {
                        purgent_core::modules::log::log(
                            "reported",
                            &format!("cloud sync pushed={pushed}"),
                        );
                    } else {
                        purgent_core::modules::log::log(
                            "reported",
                            &format!("cloud sync pushed={pushed} pending={}", failed.len()),
                        );
                    }
                }
                Err(e) => {
                    purgent_core::modules::log::log(
                        "reported",
                        &format!("cloud sync unavailable: {e}"),
                    );
                }
            }
        }
    });
}

fn record_outcome(case_db_path: &Path, report: &Report, json_path: &Path, pdf_path: &Path) {
    if let Ok(db) = CaseDb::open(case_db_path) {
        if let Err(e) = db.record_report(report, json_path, pdf_path) {
            eprintln!("[purgent] persisted report record failed: {e}");
        }
    }
}

fn finish_operation_ok(
    app: &AppHandle,
    operation_id: String,
    report_json: PathBuf,
    report_pdf: PathBuf,
) {
    let _ = app.emit(
        "operation-complete",
        OperationOutcome {
            operation_id,
            report_json,
            report_pdf,
        },
    );
}

fn finish_operation_err<E: std::fmt::Debug>(app: &AppHandle, operation_id: String, err: E) {
    let payload = serde_json::json!({
        "operation_id": operation_id,
        "error": format!("{:?}", err),
    });
    let _ = app.emit("operation-error", payload);
}

fn config_dir() -> PathBuf {
    purgent_core::modules::signing::key_path()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::temp_dir().join("purgent"))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let signing_key = load_or_create_signing_key().unwrap_or_else(|_| {
                // last-resort fallback keeps the UI usable; reports still verify locally
                vec![0u8; 32]
            });
            let report_dir = app
                .path()
                .app_data_dir()
                .map(|d| d.join("reports"))
                .unwrap_or_else(|_| config_dir().join("reports"));
            std::fs::create_dir_all(&report_dir).ok();
            let case_db_path = report_dir.join("case.sqlite3");
            if let Ok(db) = CaseDb::open(&case_db_path) {
                if let Ok(count) = db.import_directory(&report_dir) {
                    if count > 0 {
                        eprintln!("[purgent] backfilled {count} legacy report(s) into case db");
                    }
                }
            }
            let operator_id = std::fs::read_to_string(config_dir().join("operator.txt"))
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    std::env::var("USERNAME")
                        .or_else(|_| std::env::var("USER"))
                        .unwrap_or_else(|_| "operator".to_string())
                });
            let sync_config = SyncConfig::from_env();
            let sync_enabled = std::fs::read_to_string(config_dir().join("sync_optin.txt"))
                .map(|s| s.trim() == "enabled")
                .unwrap_or(false)
                && sync_config.is_some();
            app.manage(AppState {
                signing_key,
                report_dir,
                case_db_path,
                operator_id: RwLock::new(operator_id),
                sync_config,
                sync_enabled: RwLock::new(sync_enabled),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hello,
            list_devices,
            get_identity,
            set_operator,
            get_wipe_standards,
            get_erase_standards,
            wipe_image,
            wipe_device,
            erase_path,
            carve_source,
            list_reports,
            read_report,
            open_report_pdf,
            export_report_xml,
            open_report_xml,
            get_sync_status,
            set_sync_enabled,
            sync_now,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

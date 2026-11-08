use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use switch_extractor_core::{
    ExtractionSummary, KeySet, Pfs0Builder, SwitchExtractor, SwitchGameInfo,
};
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    pub cancel_flag: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyStatus {
    pub has_header_key: bool,
    pub key_count: usize,
    pub prod_keys_path: Option<String>,
    pub title_keys_path: Option<String>,
    pub search_locations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionRequest {
    pub input_path: String,
    pub output_dir: String,
    pub base_game_path: Option<String>,
    pub update_path: Option<String>,
    #[serde(default)]
    pub dlc_paths: Vec<String>,
    pub extract_ncas: bool,
    pub extract_romfs: bool,
    pub extract_exefs: bool,
    pub extract_metadata: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvertRequest {
    pub input_path: String,
    pub output_nsp_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackItem {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub stage: String,
    pub current_file: usize,
    pub total_files: usize,
    pub file_name: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPayload {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

fn default_github_repo() -> String {
    "SPIN0ZAi/Switch-1-extractor".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub default_output_dir: Option<String>,
    pub custom_prod_keys: Option<String>,
    pub custom_title_keys: Option<String>,
    pub theme: String,
    pub auto_decompress_ncz: bool,
    #[serde(default = "default_github_repo")]
    pub github_repo: String,
    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
    #[serde(default)]
    pub last_update_check: Option<String>,
    #[serde(default)]
    pub window_width: Option<u32>,
    #[serde(default)]
    pub window_height: Option<u32>,
    #[serde(default)]
    pub window_x: Option<i32>,
    #[serde(default)]
    pub window_y: Option<i32>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_output_dir: None,
            custom_prod_keys: None,
            custom_title_keys: None,
            theme: "dark".to_string(),
            auto_decompress_ncz: true,
            github_repo: default_github_repo(),
            check_updates_on_startup: true,
            last_update_check: None,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
        }
    }
}

pub fn get_settings_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata)
            .join("SwitchExtractor")
            .join("settings.json")
    } else {
        PathBuf::from("settings.json")
    }
}

fn load_app_settings() -> AppSettings {
    let path = get_settings_path();
    if path.is_file()
        && let Ok(content) = std::fs::read_to_string(&path)
        && let Ok(s) = serde_json::from_str(&content)
    {
        return s;
    }
    AppSettings::default()
}

fn emit_log(app: &AppHandle, level: &str, message: &str) {
    let now = chrono_now();
    let payload = LogPayload {
        level: level.to_string(),
        message: message.to_string(),
        timestamp: now,
    };
    let _ = app.emit("extract-log", payload);
}

fn chrono_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hours = (now / 3600) % 24;
    let mins = (now / 60) % 60;
    let secs = now % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

fn get_active_keyset() -> KeySet {
    let settings = load_app_settings();
    let prod_path = settings
        .custom_prod_keys
        .as_ref()
        .map(|s| Path::new(s.as_str()));
    let title_path = settings
        .custom_title_keys
        .as_ref()
        .map(|s| Path::new(s.as_str()));

    if prod_path.is_some() || title_path.is_some() {
        KeySet::with_paths(prod_path, title_path)
    } else {
        KeySet::auto_load()
    }
}

#[tauri::command]
pub async fn detect_keys() -> Result<KeyStatus, String> {
    let keyset = get_active_keyset();
    let search_locs: Vec<String> = KeySet::standard_search_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    Ok(KeyStatus {
        has_header_key: keyset.has_header_key(),
        key_count: keyset.count(),
        prod_keys_path: keyset
            .prod_keys_file
            .map(|p| p.to_string_lossy().to_string()),
        title_keys_path: keyset
            .title_keys_file
            .map(|p| p.to_string_lossy().to_string()),
        search_locations: search_locs,
    })
}

#[tauri::command]
pub async fn inspect_game(path: String) -> Result<SwitchGameInfo, String> {
    let file_path = PathBuf::from(&path);
    if !file_path.is_file() {
        return Err(format!("File does not exist: {}", path));
    }

    let keyset = get_active_keyset();
    let mut extractor = SwitchExtractor::new(Some(keyset));

    tokio::task::spawn_blocking(move || {
        extractor
            .inspect(&file_path)
            .map_err(|e| format!("Failed to inspect game: {}", e))
    })
    .await
    .map_err(|e| format!("Task execution failed: {}", e))?
}

#[tauri::command]
pub async fn start_extraction(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ExtractionRequest,
) -> Result<ExtractionSummary, String> {
    let cancel_flag = state.cancel_flag.clone();
    cancel_flag.store(false, Ordering::SeqCst);

    let in_path = PathBuf::from(&req.input_path);
    let out_dir = PathBuf::from(&req.output_dir);

    if !in_path.is_file() {
        return Err(format!("Input file does not exist: {}", req.input_path));
    }

    let app_clone = app.clone();
    let cancel_clone = cancel_flag.clone();

    emit_log(
        &app,
        "info",
        &format!("Starting extraction for: {}", in_path.file_name().unwrap_or_default().to_string_lossy()),
    );

    let result = tokio::task::spawn_blocking(move || {
        let keyset = get_active_keyset();
        let mut extractor = SwitchExtractor::new(Some(keyset));

        let stage_cb = |stage: &str, cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
            let percent = if sz > 0 {
                (done as f64 / sz as f64) * 100.0
            } else {
                0.0
            };

            let payload = ProgressPayload {
                stage: stage.to_string(),
                current_file: cur,
                total_files: tot,
                file_name: name.to_string(),
                bytes_done: done,
                bytes_total: sz,
                percent,
            };
            let _ = app_clone.emit("extract-progress", payload);
        };

        let cancel_check = || cancel_clone.load(Ordering::SeqCst);
        let base_path = req.base_game_path.as_deref().map(Path::new);
        let update_path = req.update_path.as_deref().map(Path::new);
        let dlc_paths: Vec<PathBuf> = req.dlc_paths.into_iter().map(PathBuf::from).collect();

        extractor.extract_all(
            &in_path,
            &out_dir,
            base_path,
            update_path,
            &dlc_paths,
            req.extract_ncas,
            req.extract_romfs,
            req.extract_exefs,
            req.extract_metadata,
            Some(&stage_cb),
            Some(&cancel_check),
        )
    })
    .await
    .map_err(|e| format!("Task crashed: {}", e))?;

    match result {
        Ok(summary) => {
            emit_log(&app, "success", "Extraction finished successfully!");
            let _ = app.emit("extract-complete", &summary);
            Ok(summary)
        }
        Err(e) => {
            let err_msg = e.to_string();
            emit_log(&app, "error", &format!("Extraction failed: {}", err_msg));
            let _ = app.emit("extract-error", &err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub async fn convert_xci_to_nsp(
    app: AppHandle,
    state: State<'_, AppState>,
    req: ConvertRequest,
) -> Result<u64, String> {
    let cancel_flag = state.cancel_flag.clone();
    cancel_flag.store(false, Ordering::SeqCst);

    let in_path = PathBuf::from(&req.input_path);
    let out_path = PathBuf::from(&req.output_nsp_path);

    if !in_path.is_file() {
        return Err(format!("XCI file does not exist: {}", req.input_path));
    }

    emit_log(&app, "info", &format!("Converting XCI to NSP: {}", in_path.display()));

    let app_clone = app.clone();
    let cancel_clone = cancel_flag.clone();

    let result = tokio::task::spawn_blocking(move || {
        let keyset = get_active_keyset();
        let mut extractor = SwitchExtractor::new(Some(keyset));

        let progress_cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
            let percent = if sz > 0 {
                (done as f64 / sz as f64) * 100.0
            } else {
                0.0
            };

            let payload = ProgressPayload {
                stage: "Converting XCI to NSP".to_string(),
                current_file: cur,
                total_files: tot,
                file_name: name.to_string(),
                bytes_done: done,
                bytes_total: sz,
                percent,
            };
            let _ = app_clone.emit("convert-progress", payload);
        };

        let cancel_check = || cancel_clone.load(Ordering::SeqCst);

        extractor.convert_xci_to_nsp(&in_path, &out_path, Some(&progress_cb), Some(&cancel_check))
    })
    .await
    .map_err(|e| format!("Task execution failed: {}", e))?;

    match result {
        Ok(bytes) => {
            emit_log(&app, "success", &format!("Direct XCI to NSP converted ({} bytes)", bytes));
            let _ = app.emit("convert-complete", bytes);
            Ok(bytes)
        }
        Err(e) => {
            let err_msg = e.to_string();
            emit_log(&app, "error", &format!("Conversion failed: {}", err_msg));
            let _ = app.emit("convert-error", &err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub async fn pack_pfs0(
    app: AppHandle,
    state: State<'_, AppState>,
    files: Vec<PackItem>,
    output_path: String,
) -> Result<u64, String> {
    let cancel_flag = state.cancel_flag.clone();
    cancel_flag.store(false, Ordering::SeqCst);

    let out_file = PathBuf::from(&output_path);
    let pack_files: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|f| (f.name, PathBuf::from(f.path)))
        .collect();

    emit_log(&app, "info", &format!("Packing {} files into {}", pack_files.len(), out_file.display()));

    let app_clone = app.clone();
    let cancel_clone = cancel_flag.clone();

    let result = tokio::task::spawn_blocking(move || {
        let progress_cb = |cur: usize, tot: usize, name: &str, done: u64, sz: u64| {
            let percent = if sz > 0 {
                (done as f64 / sz as f64) * 100.0
            } else {
                0.0
            };

            let payload = ProgressPayload {
                stage: "Packing PFS0 / NSP".to_string(),
                current_file: cur,
                total_files: tot,
                file_name: name.to_string(),
                bytes_done: done,
                bytes_total: sz,
                percent,
            };
            let _ = app_clone.emit("pack-progress", payload);
        };

        let cancel_check = || cancel_clone.load(Ordering::SeqCst);

        Pfs0Builder::build(&pack_files, &out_file, Some(&progress_cb), Some(&cancel_check))
    })
    .await
    .map_err(|e| format!("Task execution failed: {}", e))?;

    match result {
        Ok(bytes) => {
            emit_log(&app, "success", &format!("Pack complete: {} bytes written", bytes));
            Ok(bytes)
        }
        Err(e) => {
            let err_msg = e.to_string();
            emit_log(&app, "error", &format!("Packing failed: {}", err_msg));
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub fn cancel_operation(state: State<'_, AppState>) {
    state.cancel_flag.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let target = if p.is_file() {
        p.parent().unwrap_or(&p).to_path_buf()
    } else {
        p
    };

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_settings() -> AppSettings {
    load_app_settings()
}

#[tauri::command]
pub fn save_settings(settings: AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json_str = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json_str).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_initial_file() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let candidate = &args[1];
        let p = Path::new(candidate);
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

#[tauri::command]
pub fn window_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn window_toggle_maximize(window: tauri::Window) -> Result<(), String> {
    let is_max = window.is_maximized().map_err(|e| e.to_string())?;
    if is_max {
        window.unmaximize().map_err(|e| e.to_string())
    } else {
        window.maximize().map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn window_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn find_matching_base_game(update_path: String) -> Result<Option<String>, String> {
    let u_path = PathBuf::from(&update_path);
    if !u_path.is_file() {
        return Ok(None);
    }

    let parent = match u_path.parent() {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut dirs_to_check = vec![parent.to_path_buf()];
    if let Some(grandparent) = parent.parent() {
        dirs_to_check.push(grandparent.to_path_buf());
    }

    let keyset = get_active_keyset();
    let mut extractor = SwitchExtractor::new(Some(keyset));

    let update_info = match extractor.inspect(&u_path) {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };

    let target_base_tid = if update_info.title_id.ends_with("800") && update_info.title_id.len() == 16 {
        format!("{}000", &update_info.title_id[..13])
    } else {
        String::new()
    };

    for dir in dirs_to_check {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path != u_path {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if matches!(ext.as_str(), "nsp" | "xci" | "nsz" | "xcz") {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !target_base_tid.is_empty() && name.contains(&target_base_tid) {
                            return Ok(Some(path.to_string_lossy().to_string()));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn find_matching_update(base_path: String) -> Result<Option<String>, String> {
    let b_path = PathBuf::from(&base_path);
    if !b_path.is_file() {
        return Ok(None);
    }

    let parent = match b_path.parent() {
        Some(p) => p,
        None => return Ok(None),
    };

    let mut dirs_to_check = vec![parent.to_path_buf()];
    if let Some(grandparent) = parent.parent() {
        dirs_to_check.push(grandparent.to_path_buf());
    }

    let keyset = get_active_keyset();
    let mut extractor = SwitchExtractor::new(Some(keyset));

    let base_info = match extractor.inspect(&b_path) {
        Ok(i) => i,
        Err(_) => return Ok(None),
    };

    let target_upd_tid = if base_info.title_id.ends_with("000") && base_info.title_id.len() == 16 {
        format!("{}800", &base_info.title_id[..13])
    } else {
        String::new()
    };

    for dir in dirs_to_check {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path != b_path {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if matches!(ext.as_str(), "nsp" | "xci" | "nsz" | "xcz") {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let name_low = name.to_lowercase();
                        if (!target_upd_tid.is_empty() && name.contains(&target_upd_tid))
                            || name_low.contains("[upd")
                            || (name_low.contains("update") && !name_low.contains("base"))
                        {
                            return Ok(Some(path.to_string_lossy().to_string()));
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

#[tauri::command]
pub async fn find_matching_dlcs(base_path: String) -> Result<Vec<String>, String> {
    let b_path = PathBuf::from(&base_path);
    if !b_path.is_file() {
        return Ok(Vec::new());
    }

    let parent = match b_path.parent() {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    let mut dirs_to_check = vec![parent.to_path_buf()];
    if let Some(grandparent) = parent.parent() {
        dirs_to_check.push(grandparent.to_path_buf());
    }

    let keyset = get_active_keyset();
    let mut extractor = SwitchExtractor::new(Some(keyset));

    let base_info = match extractor.inspect(&b_path) {
        Ok(i) => i,
        Err(_) => return Ok(Vec::new()),
    };

    let base_prefix = if base_info.title_id.len() == 16 {
        base_info.title_id[..13].to_string()
    } else {
        String::new()
    };

    let mut found_dlcs = Vec::new();

    for dir in dirs_to_check {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path != b_path {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    if matches!(ext.as_str(), "nsp" | "xci" | "nsz" | "xcz") {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        let name_low = name.to_lowercase();

                        let is_dlc_match = if !base_prefix.is_empty() && name.contains(&base_prefix) {
                            !name.contains(&format!("{}000", base_prefix))
                                && !name.contains(&format!("{}800", base_prefix))
                        } else {
                            name_low.contains("[dlc") || name_low.contains(" dlc") || name_low.contains("_dlc")
                        };

                        if is_dlc_match {
                            let p_str = path.to_string_lossy().to_string();
                            if !found_dlcs.contains(&p_str) {
                                found_dlcs.push(p_str);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(found_dlcs)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_name: String,
    pub release_notes: String,
    pub release_url: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: Option<u64>,
    pub published_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

fn parse_version_parts(v: &str) -> Vec<u64> {
    let clean = v.trim_start_matches(['v', 'V']);
    clean
        .split(['.', '-', '+'])
        .filter_map(|s| s.parse::<u64>().ok())
        .collect()
}

pub fn is_version_newer(current: &str, latest: &str) -> bool {
    let curr_parts = parse_version_parts(current);
    let late_parts = parse_version_parts(latest);

    let max_len = curr_parts.len().max(late_parts.len());
    for i in 0..max_len {
        let c = curr_parts.get(i).copied().unwrap_or(0);
        let l = late_parts.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        } else if l < c {
            return false;
        }
    }
    false
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn check_for_updates(repo: Option<String>) -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let target_repo = repo
        .filter(|r| !r.trim().is_empty())
        .unwrap_or_else(|| "switch-extractor/switch-extractor-desktop".to_string());
    let clean_repo = target_repo.trim_matches('/');

    let url = format!("https://api.github.com/repos/{}/releases/latest", clean_repo);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let resp = client
        .get(&url)
        .header("User-Agent", format!("SwitchExtractor/{}", current_version))
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .map_err(|e| format!("Network error querying GitHub: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!(
            "GitHub repository returned HTTP {}: {}",
            resp.status(),
            resp.status().canonical_reason().unwrap_or("Not Found or Rate Limited")
        ));
    }

    let release = resp
        .json::<GitHubRelease>()
        .await
        .map_err(|e| format!("Failed to parse GitHub release data: {}", e))?;

    let latest_version = release.tag_name.clone();
    let is_newer = is_version_newer(&current_version, &latest_version);

    // Look for Windows executable asset (e.g. SwitchExtractor.exe)
    let matching_asset = release
        .assets
        .iter()
        .find(|a| {
            let n = a.name.to_lowercase();
            n.contains("switchextractor") && n.ends_with(".exe")
        })
        .or_else(|| {
            release.assets.iter().find(|a| a.name.to_lowercase().ends_with(".exe"))
        })
        .or_else(|| {
            release.assets.iter().find(|a| a.name.to_lowercase().ends_with(".zip"))
        });

    let (download_url, asset_name, asset_size) = match matching_asset {
        Some(a) => (Some(a.browser_download_url.clone()), Some(a.name.clone()), Some(a.size)),
        None => (None, None, None),
    };

    Ok(UpdateInfo {
        update_available: is_newer,
        current_version,
        latest_version,
        release_name: release.name.unwrap_or(release.tag_name),
        release_notes: release.body.unwrap_or_default(),
        release_url: release.html_url,
        download_url,
        asset_name,
        asset_size,
        published_at: release.published_at.unwrap_or_default(),
    })
}

#[tauri::command]
pub async fn download_and_install_update(
    app: AppHandle,
    download_url: String,
) -> Result<String, String> {
    let current_version = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let resp = client
        .get(&download_url)
        .header("User-Agent", format!("SwitchExtractor/{}", current_version))
        .send()
        .await
        .map_err(|e| format!("Failed to download update: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with status: {}", resp.status()));
    }

    let total_bytes = resp.content_length().unwrap_or(0);
    let temp_dir = std::env::temp_dir();
    let temp_file_path = temp_dir.join("SwitchExtractor_update.exe");

    let mut out_file = tokio::fs::File::create(&temp_file_path)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Download stream error: {}", e))?;
        use tokio::io::AsyncWriteExt;
        out_file
            .write_all(&chunk)
            .await
            .map_err(|e| format!("Disk write error: {}", e))?;

        downloaded += chunk.len() as u64;
        let percent = if total_bytes > 0 {
            (downloaded as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };

        let _ = app.emit(
            "update-download-progress",
            UpdateDownloadProgress {
                bytes_downloaded: downloaded,
                total_bytes,
                percent,
            },
        );
    }

    use tokio::io::AsyncWriteExt;
    out_file
        .flush()
        .await
        .map_err(|e| format!("Flush error: {}", e))?;
    drop(out_file);

    // Apply the update on Windows
    let current_exe = std::env::current_exe().map_err(|e| format!("Could not get current exe: {}", e))?;

    let old_exe = current_exe.with_extension("exe.old");
    let _ = std::fs::remove_file(&old_exe);

    if std::fs::rename(&current_exe, &old_exe).is_ok() {
        if let Err(e) = std::fs::copy(&temp_file_path, &current_exe) {
            let _ = std::fs::rename(&old_exe, &current_exe);
            return Err(format!("Failed to install new executable: {}", e));
        }

        let _ = std::process::Command::new(&current_exe).spawn();
        std::process::exit(0);
    } else {
        let script = format!(
            "timeout /t 1 /nobreak > nul & move /y \"{}\" \"{}\" & start \"\" \"{}\"",
            temp_file_path.display(),
            current_exe.display(),
            current_exe.display()
        );

        let _ = std::process::Command::new("cmd")
            .args(["/C", &script])
            .spawn();

        std::process::exit(0);
    }
}


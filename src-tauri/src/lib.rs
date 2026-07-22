use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use reqwest::blocking::Client;
use rusqlite::{types::ValueRef, Connection};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, LazyLock, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};
use tempfile::{tempdir, TempDir};
use zip::result::ZipError;
use zip::ZipArchive;

static CACHE_STATE: LazyLock<Mutex<CacheState>> =
    LazyLock::new(|| Mutex::new(CacheState::default()));
const BACKUP_VIRTUAL_ROOT: &str = "__manifest_backup__";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ZipScanSummary {
    source_path: String,
    source_mode: String,
    source_zips: Vec<String>,
    zip_count: usize,
    batch_root: Option<String>,
    app_count: usize,
    file_count: usize,
    cache_hit: bool,
    apps: Vec<AppSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppSummary {
    source_zip: String,
    app_id: String,
    display_name: String,
    subtitle: String,
    app_kind: String,
    logo_text: String,
    logo_color: String,
    total_files: usize,
    candidate_files: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CandidateFile {
    source_zip: String,
    app_id: String,
    inner_path: String,
    file_type: String,
    parameter_scope: String,
    size: u64,
    parse_supported: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ParseResult {
    source_zip: String,
    app_id: String,
    inner_path: String,
    file_type: String,
    parse_status: String,
    parsed_data: Value,
    meta: Value,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResult {
    output_path: String,
    item_count: usize,
    bytes_written: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanProgressPayload {
    stage: String,
    message: String,
    current: usize,
    total: usize,
    current_zip: Option<String>,
    percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinUniqueIdResult {
    uid: String,
    sec_uid: String,
    unique_id: String,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToutiaoSecuidResult {
    tt_uid: String,
    tt_secuid: String,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinRequestParamsResult {
    source_zip: String,
    source_plist_path: String,
    source_cookie_path: Option<String>,
    sec_user_id: String,
    cookie_header: String,
    header_count: usize,
    header_text: String,
    headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinPasswordStatusResult {
    source_zip: String,
    source_cookie_path: Option<String>,
    session_id: String,
    has_password: Option<bool>,
    account_name: Option<String>,
    register_time: Option<String>,
    bindings: DouyinSessionBindings,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinSessionBindings {
    summary: String,
    toutiao: String,
    toutiao_platform_screen_name: String,
    qq: String,
    qq_platform_screen_name: String,
    google: String,
    google_platform_screen_name: String,
    apple_id: String,
    apple_id_platform_screen_name: String,
    wechat: String,
    wechat_platform_screen_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinCertificationStatusResult {
    source_zip: String,
    source_plist_path: Option<String>,
    is_verified: Option<bool>,
    account_name: Option<String>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToutiaoCertificationStatusResult {
    source_zip: String,
    source_plist_path: Option<String>,
    source_cookie_path: Option<String>,
    act_token: String,
    odin_tt: String,
    is_verified: Option<bool>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToutiaoTokenStatusResult {
    source_zip: String,
    source_plist_path: Option<String>,
    source_cookie_path: Option<String>,
    token_preview: String,
    odin_tt_preview: String,
    device_id: String,
    iid: String,
    nickname: Option<String>,
    uid: Option<String>,
    register_time: Option<String>,
    http_status: Option<u16>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinFunctionItem {
    func_name: String,
    func_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinTokenStatusResult {
    source_zip: String,
    source_plist_path: Option<String>,
    source_cookie_path: Option<String>,
    token_preview: String,
    odin_tt_preview: String,
    local_phone_number: Option<String>,
    status: String,
    valid_endpoint_count: usize,
    endpoints: Vec<DouyinTokenEndpointResult>,
    functions: Vec<DouyinFunctionItem>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinTokenEndpointResult {
    name: String,
    url: String,
    http_status: Option<u16>,
    status_code: Option<i64>,
    status: String,
    message: Option<String>,
    uid: Option<String>,
    sec_uid: Option<String>,
    nickname: Option<String>,
    phone_number: Option<String>,
    register_time: Option<String>,
    aweme_count: Option<String>,
    following_count: Option<String>,
    liked_count: Option<String>,
    functions: Vec<DouyinFunctionItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinAccountCredentialItem {
    uid: String,
    nickname: String,
    sec_uid: String,
    unique_id: String,
    short_id: String,
    session_id: String,
    session_id_preview: String,
    access_token: String,
    access_token_preview: String,
    open_id: String,
    open_id_preview: String,
    auth_time_label: String,
    is_current: bool,
    phone_number: String,
    register_time: String,
    aweme_count: String,
    following_count: String,
    liked_count: String,
    bindings: DouyinSessionBindings,
    has_password: Option<bool>,
    is_verified: Option<bool>,
    normal_functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DouyinAccountCredentialResult {
    source_zip: String,
    source_plist_path: Option<String>,
    source_cookie_path: Option<String>,
    current_session_id_preview: String,
    current_token_preview: String,
    current_odin_tt_preview: String,
    account_count: usize,
    accounts: Vec<DouyinAccountCredentialItem>,
    status: String,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct AppStats {
    total_files: usize,
    candidate_files: usize,
}

#[derive(Debug, Clone)]
struct IndexedEntry {
    display_path: String,
    app_id: String,
    file_type: &'static str,
    parameter_scope: &'static str,
    size: u64,
}

struct AppPresentation {
    display_name: String,
    subtitle: String,
    app_kind: String,
    logo_text: String,
    logo_color: String,
}

struct ScanInput {
    source_mode: String,
    zip_paths: Vec<String>,
}

#[derive(Debug, Default)]
struct CacheState {
    scan_cache: BTreeMap<String, ZipScanSummary>,
    files_cache: BTreeMap<String, Vec<CandidateFile>>,
    parse_cache: BTreeMap<String, ParseResult>,
    app_file_path_indexes: BTreeMap<String, Vec<String>>,
}

struct BackupManifestContext {
    _temp_dir: Option<TempDir>,
    connection: Connection,
    base_dir: String,
}

#[derive(Debug, Clone, Default)]
struct DouyinLocalAccountIdentity {
    uid: String,
    nickname: String,
    sec_uid: String,
    unique_id: String,
    short_id: String,
    phone_number: String,
    register_time: String,
    aweme_count: String,
    following_count: String,
    liked_count: String,
    bindings: DouyinSessionBindings,
    has_password: Option<bool>,
    is_verified: Option<bool>,
    normal_functions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct DouyinTokenClusterEntry {
    access_token: String,
    open_id: String,
    sec_uid: String,
    auth_time_label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ParsedDouyinPasswordStatus {
    has_password: Option<bool>,
    screen_name: Option<String>,
    register_time: Option<String>,
    bindings: DouyinSessionBindings,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedDouyinCertificationStatus {
    is_verified: Option<bool>,
    screen_name: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedToutiaoCertificationStatus {
    is_verified: Option<bool>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedToutiaoTokenCheck {
    is_valid: Option<bool>,
    message: Option<String>,
    nickname: Option<String>,
    uid: Option<String>,
    register_time: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedDouyinTokenCheck {
    is_valid: Option<bool>,
    status_code: Option<i64>,
    message: Option<String>,
    uid: Option<String>,
    sec_uid: Option<String>,
    nickname: Option<String>,
    phone_number: Option<String>,
    register_time: Option<String>,
    aweme_count: Option<String>,
    following_count: Option<String>,
    liked_count: Option<String>,
    functions: Vec<DouyinFunctionItem>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct ParsedDouyinProfileOtherIdentity {
    uid: Option<String>,
    sec_uid: Option<String>,
    unique_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum DouyinTokenEndpoint {
    SafetyPortrait,
    ProfileSelf,
}

#[derive(Debug, Clone, Copy)]
enum DouyinSessionBindingPlatform {
    Toutiao,
    Qq,
    Google,
    AppleId,
    Wechat,
}

impl DouyinSessionBindingPlatform {
    fn label(self) -> &'static str {
        match self {
            Self::Toutiao => "头条",
            Self::Qq => "QQ",
            Self::Google => "谷歌",
            Self::AppleId => "ID",
            Self::Wechat => "微信",
        }
    }
}

impl DouyinTokenEndpoint {
    fn name(self) -> &'static str {
        match self {
            Self::SafetyPortrait => "safety_portrait",
            Self::ProfileSelf => "profile_self",
        }
    }

    fn base_url(self) -> &'static str {
        match self {
            Self::SafetyPortrait => {
                "https://api5-normal-c-hl.amemv.com/aweme/v3/user/safety/portrait/"
            }
            Self::ProfileSelf => "https://api3-core-c-hl.amemv.com/aweme/v1/user/profile/self/",
        }
    }

    fn sdk_version(self) -> &'static str {
        match self {
            Self::SafetyPortrait => "2",
            Self::ProfileSelf => "1",
        }
    }
}

#[tauri::command]
async fn scan_path(app: AppHandle, input_path: String) -> Result<ZipScanSummary, String> {
    tauri::async_runtime::spawn_blocking(move || scan_path_impl(&app, input_path))
        .await
        .map_err(|error| format!("scan_task_join_failed: {error}"))?
}

fn scan_path_impl(app: &AppHandle, input_path: String) -> Result<ZipScanSummary, String> {
    let scan_input = resolve_scan_input(&input_path)?;
    let scan_cache_key = build_scan_cache_key(&input_path, &scan_input.zip_paths)?;
    let total_zips = scan_input.zip_paths.len().max(1);

    emit_scan_progress(app, "scan_path", "正在准备扫描路径...", 0, total_zips, None);

    if let Some(cached) = cache_get_scan(&scan_cache_key)? {
        emit_scan_progress(
            app,
            "scan_path",
            &format!("扫描命中缓存，共 {} 个 ZIP", cached.zip_count),
            cached.zip_count,
            cached.zip_count.max(1),
            cached.source_zips.first().cloned(),
        );
        return Ok(with_scan_cache_hit(cached, true));
    }

    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let spawn_limit = 20.min(4.max(cpu_cores * 2));
    let thread_count = spawn_limit.min(total_zips);

    struct ThreadResult {
        file_count: usize,
        apps: BTreeMap<(String, String), AppStats>,
        batch_root: Option<String>,
        has_mixed: bool,
    }

    let progress = Arc::new(AtomicUsize::new(0));
    let chunk_ranges = build_chunk_ranges(scan_input.zip_paths.len(), thread_count);
    let results = Arc::new(Mutex::new(Vec::with_capacity(chunk_ranges.len())));

    std::thread::scope(|scope| {
        for (start, end) in chunk_ranges {
            let chunk_zips: Vec<String> = scan_input.zip_paths[start..end].to_vec();
            let progress = Arc::clone(&progress);
            let results = Arc::clone(&results);
            let app_handle = app.clone();

            scope.spawn(move || {
                let mut file_count = 0usize;
                let mut apps: BTreeMap<(String, String), AppStats> = BTreeMap::new();
                let mut batch_root = None::<String>;
                let mut has_mixed = false;

                for zip_path in &chunk_zips {
                    let entries = match collect_candidate_entries(zip_path) {
                        Ok(entries) => entries,
                        Err(_e) => {
                            let _ = progress.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    };

                    for entry in entries {
                        let Some((entry_batch_root, app_id, _)) =
                            split_entry_path(&entry.display_path)
                        else {
                            continue;
                        };

                        file_count += 1;

                        if let Some(value) = entry_batch_root.as_ref() {
                            if let Some(current) = batch_root.as_ref() {
                                if current != value {
                                    has_mixed = true;
                                }
                            } else {
                                batch_root = Some(value.clone());
                            }
                        }

                        let stats = apps.entry((zip_path.clone(), app_id)).or_default();
                        stats.total_files += 1;
                        stats.candidate_files += 1;
                    }

                    let done = progress.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = app_handle.emit(
                        "scan-progress",
                        serde_json::json!({
                            "stage": "scan_path",
                            "message": format!("多线程扫描 {}/{}", done, total_zips),
                            "current": done,
                            "total": total_zips,
                            "currentZip": zip_path,
                            "percent": ((done as f64 / total_zips as f64) * 100.0).round() as u32,
                        }),
                    );
                }

                let mut locked = results.lock().unwrap();
                locked.push(ThreadResult {
                    file_count,
                    apps,
                    batch_root,
                    has_mixed,
                });
            });
        }
    });

    let mut file_count = 0usize;
    let mut apps: BTreeMap<(String, String), AppStats> = BTreeMap::new();
    let mut batch_root = None::<String>;
    let mut has_mixed_batch_root = false;

    for r in results.lock().unwrap().drain(..) {
        file_count += r.file_count;
        for ((zip, app_id), stats) in r.apps {
            let entry = apps.entry((zip, app_id)).or_default();
            entry.total_files += stats.total_files;
            entry.candidate_files += stats.candidate_files;
        }
        if let Some(b) = &r.batch_root {
            if let Some(current) = batch_root.as_ref() {
                if current != b {
                    has_mixed_batch_root = true;
                }
            } else {
                batch_root = Some(b.clone());
            }
        }
        if r.has_mixed {
            has_mixed_batch_root = true;
        }
    }

    if has_mixed_batch_root {
        batch_root = None;
    }

    let app_summaries = apps
        .into_iter()
        .filter(|((_, app_id), stats)| is_keychain_app(app_id) || stats.candidate_files > 0)
        .map(|((source_zip, app_id), stats)| {
            let presentation = build_app_presentation(&app_id);
            AppSummary {
                source_zip,
                app_id,
                display_name: presentation.display_name,
                subtitle: presentation.subtitle,
                app_kind: presentation.app_kind,
                logo_text: presentation.logo_text,
                logo_color: presentation.logo_color,
                total_files: stats.total_files,
                candidate_files: stats.candidate_files,
            }
        })
        .collect::<Vec<_>>();

    let mut app_summaries = app_summaries;
    app_summaries.sort_by(|left, right| {
        left.display_name
            .cmp(&right.display_name)
            .then(left.app_id.cmp(&right.app_id))
            .then(left.source_zip.cmp(&right.source_zip))
    });

    let summary = ZipScanSummary {
        source_path: input_path,
        source_mode: scan_input.source_mode,
        source_zips: scan_input.zip_paths.clone(),
        zip_count: scan_input.zip_paths.len(),
        batch_root,
        app_count: app_summaries.len(),
        file_count,
        cache_hit: false,
        apps: app_summaries,
    };
    emit_scan_progress(
        app,
        "scan_path",
        &format!("扫描完成，共 {} 个 ZIP", summary.zip_count),
        summary.zip_count,
        summary.zip_count.max(1),
        summary.source_zips.last().cloned(),
    );
    let _ = cache_put_scan(scan_cache_key, summary.clone());
    Ok(summary)
}

#[tauri::command]
fn list_files(zip_path: String, app_id: String) -> Result<Vec<CandidateFile>, String> {
    let zip_cache_key = build_zip_cache_key(&zip_path)?;
    let files_cache_key = build_files_cache_key(&zip_cache_key, &app_id);

    if let Some(cached) = cache_get_files(&files_cache_key)? {
        return Ok(cached);
    }

    let mut files = collect_candidate_entries(&zip_path)?
        .into_iter()
        .filter(|entry| entry.app_id == app_id)
        .map(|entry| CandidateFile {
            source_zip: zip_path.clone(),
            app_id: entry.app_id,
            inner_path: entry.display_path,
            file_type: entry.file_type.to_string(),
            parameter_scope: entry.parameter_scope.to_string(),
            size: entry.size,
            parse_supported: is_parse_supported(entry.file_type),
        })
        .collect::<Vec<_>>();

    files.sort_by(|left, right| {
        let scope_order = parameter_scope_priority(&left.parameter_scope)
            .cmp(&parameter_scope_priority(&right.parameter_scope));
        if scope_order == std::cmp::Ordering::Equal {
            left.inner_path.cmp(&right.inner_path)
        } else {
            scope_order
        }
    });
    cache_put_files(files_cache_key, files.clone())?;
    Ok(files)
}

#[tauri::command]
fn parse_file(zip_path: String, inner_path: String) -> Result<ParseResult, String> {
    let zip_cache_key = build_zip_cache_key(&zip_path)?;
    let parse_cache_key = build_parse_cache_key(&zip_cache_key, &inner_path);

    if let Some(cached) = cache_get_parse(&parse_cache_key)? {
        return Ok(with_parse_cache_hit(cached, true));
    }

    let (_, app_id, sandbox_path) =
        split_entry_path(&inner_path).ok_or_else(|| "invalid_path_layout".to_string())?;
    let file_type = detect_file_type(&sandbox_path).to_string();
    let parameter_scope = classify_parameter_scope(&sandbox_path);

    if !should_analyze_file(parameter_scope, &file_type) {
        return Ok(ParseResult {
            source_zip: zip_path.clone(),
            app_id,
            inner_path,
            file_type,
            parse_status: "unsupported".to_string(),
            parsed_data: Value::Null,
            meta: json!({ "parameterScope": parameter_scope }),
            error: Some("unsupported_parameter_scope".to_string()),
        });
    }

    let result = match file_type.as_str() {
        "plist" => parse_plist_file(&zip_path, &inner_path, &app_id, &file_type),
        "json" => parse_json_file(&zip_path, &inner_path, &app_id, &file_type),
        "sqlite" => parse_sqlite_file(&zip_path, &inner_path, &app_id, &file_type),
        "binarycookies" => parse_binarycookies_file(&zip_path, &inner_path, &app_id, &file_type),
        _ => Ok(ParseResult {
            source_zip: zip_path.clone(),
            app_id,
            inner_path,
            file_type,
            parse_status: "unsupported".to_string(),
            parsed_data: Value::Null,
            meta: json!({}),
            error: Some("unsupported_file_type".to_string()),
        }),
    }?;

    cache_put_parse(parse_cache_key, result.clone())?;
    Ok(with_parse_cache_hit(result, false))
}

#[tauri::command]
fn export_file_result(
    zip_path: String,
    inner_path: String,
    output_path: Option<String>,
) -> Result<ExportResult, String> {
    let parsed = parse_file(zip_path.clone(), inner_path.clone())?;
    let file_name = sanitize_file_name(&inner_path);
    let zip_prefix = export_source_prefix(&zip_path);
    let output_path = resolve_export_path(
        &zip_path,
        output_path,
        &format!("{zip_prefix}__{file_name}.json"),
    )?;
    let payload = json!({
        "version": 2,
        "sourceZip": zip_path,
        "innerPath": inner_path,
        "exportedAt": now_unix_timestamp(),
        "result": parsed,
    });
    write_json_export(output_path, &payload, 1)
}

#[tauri::command]
fn export_app_result(
    zip_path: String,
    app_id: String,
    output_path: Option<String>,
) -> Result<ExportResult, String> {
    let files = list_files(zip_path.clone(), app_id.clone())?;
    let focused_files = files
        .into_iter()
        .filter(|file| file.parameter_scope != "other")
        .collect::<Vec<_>>();
    let mut results = Vec::new();

    for file in &focused_files {
        results.push(parse_file(zip_path.clone(), file.inner_path.clone())?);
    }

    let output_path = resolve_export_path(
        &zip_path,
        output_path,
        &format!(
            "{}__{}.json",
            export_source_prefix(&zip_path),
            sanitize_file_name(&app_id)
        ),
    )?;
    let payload = json!({
        "version": 2,
        "sourceZip": zip_path,
        "appId": app_id,
        "exportedAt": now_unix_timestamp(),
        "focusScopes": ["preferences", "cookies", "webkit", "keychain"],
        "fileCount": focused_files.len(),
        "files": results,
    });
    write_json_export(output_path, &payload, focused_files.len())
}

const ALLOWED_ZIP_TARGET_SUBDIRS: &[&str] = &[
    "online",
    "offline",
    "normal_functions",
    "limited_functions",
    "douyin_online",
    "toutiao_online",
];

#[derive(Clone, Copy)]
enum ZipTransferMode {
    Move,
    Copy,
}

fn is_allowed_zip_target_subdir(target_subdir: &str) -> bool {
    ALLOWED_ZIP_TARGET_SUBDIRS.contains(&target_subdir)
}

fn transfer_zip_files_impl(
    zip_paths: Vec<String>,
    target_subdir: String,
    mode: ZipTransferMode,
) -> Result<Vec<String>, String> {
    if !is_allowed_zip_target_subdir(&target_subdir) {
        return Err(format!("无效目标目录: {target_subdir}"));
    }

    let action = match mode {
        ZipTransferMode::Move => "移动",
        ZipTransferMode::Copy => "复制",
    };
    let mut transferred = Vec::new();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();

    for zip_path in &zip_paths {
        if !seen.insert(zip_path.clone()) {
            continue;
        }
        let src = Path::new(zip_path);
        if !src.is_file() {
            errors.push(format!("源文件不存在或不是文件: {zip_path}"));
            continue;
        }
        if !src
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
        {
            errors.push(format!("仅支持移动或复制 ZIP 文件: {zip_path}"));
            continue;
        }
        let Some(parent) = src.parent() else {
            errors.push(format!("无法获取父目录: {zip_path}"));
            continue;
        };
        let filename = src
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.zip");
        let destination_dir = parent.join(&target_subdir);
        if let Err(error) = fs::create_dir_all(&destination_dir) {
            errors.push(format!(
                "创建目录失败 {}: {error}",
                destination_dir.display()
            ));
            continue;
        }
        let destination = destination_dir.join(filename);
        if destination.exists() {
            errors.push(format!("目标文件已存在: {}", destination.display()));
            continue;
        }

        let transfer_result = match mode {
            ZipTransferMode::Move => fs::rename(src, &destination),
            ZipTransferMode::Copy => fs::copy(src, &destination).map(|_| ()),
        };
        match transfer_result {
            Ok(()) => transferred.push(destination.display().to_string()),
            Err(error) => errors.push(format!(
                "{action}失败 {} -> {}: {error}",
                src.display(),
                destination.display()
            )),
        }
    }

    if transferred.is_empty() && !errors.is_empty() {
        Err(errors.join("；"))
    } else if !errors.is_empty() {
        Ok(vec![format!(
            "{action} {} 个文件成功，{} 个失败：{}",
            transferred.len(),
            errors.len(),
            errors.join("；")
        )])
    } else {
        Ok(vec![format!(
            "成功{action} {} 个文件到 {target_subdir}/",
            transferred.len()
        )])
    }
}

fn move_zip_files_impl(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    transfer_zip_files_impl(zip_paths, target_subdir, ZipTransferMode::Move)
}

fn copy_zip_files_impl(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    transfer_zip_files_impl(zip_paths, target_subdir, ZipTransferMode::Copy)
}

#[tauri::command]
async fn move_zip_files(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || move_zip_files_impl(zip_paths, target_subdir))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

#[tauri::command]
async fn copy_zip_files(
    zip_paths: Vec<String>,
    target_subdir: String,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || copy_zip_files_impl(zip_paths, target_subdir))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

#[tauri::command]
async fn resolve_douyin_unique_id(sec_uid: String) -> Result<DouyinUniqueIdResult, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_douyin_unique_id_impl(sec_uid))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

fn resolve_douyin_unique_id_impl(sec_uid: String) -> Result<DouyinUniqueIdResult, String> {
    let trimmed_sec_uid = sec_uid.trim().to_string();
    if trimmed_sec_uid.is_empty() {
        return Ok(DouyinUniqueIdResult {
            uid: String::new(),
            sec_uid: String::new(),
            unique_id: String::new(),
            status: "empty".to_string(),
            error: Some("empty_sec_uid".to_string()),
        });
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("douyin_unique_id_client_failed: {error}"))?;

    let response = client
        .get("https://imdesktop.douyin.com/aweme/v1/web/user/profile/other/")
        .query(&[
            ("aid", "339757"),
            ("device_id", "7184690798967999755"),
            ("version_name", "1.0.0"),
            ("device_platform", "win32"),
            ("sec_user_id", trimmed_sec_uid.as_str()),
        ])
        .send()
        .map_err(|error| format!("douyin_unique_id_request_failed: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Ok(DouyinUniqueIdResult {
            uid: String::new(),
            sec_uid: trimmed_sec_uid,
            unique_id: String::new(),
            status: "http_error".to_string(),
            error: Some(format!("http_status_{status}")),
        });
    }

    let payload = response
        .json::<Value>()
        .map_err(|error| format!("douyin_unique_id_decode_failed: {error}"))?;
    let api_status = payload
        .get("status_code")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let parsed_identity = parse_douyin_profile_other_payload(&payload, &trimmed_sec_uid);
    let uid = parsed_identity.uid.unwrap_or_default();
    let sec_uid = parsed_identity
        .sec_uid
        .unwrap_or_else(|| trimmed_sec_uid.clone());
    let unique_id = parsed_identity.unique_id.unwrap_or_default();

    if api_status == 0 && !unique_id.is_empty() {
        return Ok(DouyinUniqueIdResult {
            uid,
            sec_uid,
            unique_id,
            status: "ok".to_string(),
            error: None,
        });
    }

    Ok(DouyinUniqueIdResult {
        uid,
        sec_uid,
        unique_id,
        status: "api_error".to_string(),
        error: Some(
            payload
                .get("status_msg")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("status_code_{api_status}")),
        ),
    })
}

#[tauri::command]
async fn resolve_toutiao_secuid(tt_uid: String) -> Result<ToutiaoSecuidResult, String> {
    tauri::async_runtime::spawn_blocking(move || resolve_toutiao_secuid_impl(tt_uid))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

fn resolve_toutiao_secuid_impl(tt_uid: String) -> Result<ToutiaoSecuidResult, String> {
    let trimmed_uid = tt_uid.trim().to_string();
    if trimmed_uid.is_empty() {
        return Ok(ToutiaoSecuidResult {
            tt_uid: String::new(),
            tt_secuid: String::new(),
            status: "empty".to_string(),
            error: Some("empty_tt_uid".to_string()),
        });
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|error| format!("toutiao_secuid_client_failed: {error}"))?;

    let url = format!(
        "https://www.toutiao.com/c/user/{}/?source=m_redirect",
        trimmed_uid
    );
    let response = client
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0(compatible; Baiduspider/2.0;+http://www.baidu.com/search/spider.html)",
        )
        .header("Cookie", "ttwid=78473")
        .send()
        .map_err(|error| format!("toutiao_secuid_request_failed: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        return Ok(ToutiaoSecuidResult {
            tt_uid: trimmed_uid,
            tt_secuid: String::new(),
            status: "http_error".to_string(),
            error: Some(format!("http_status_{status}")),
        });
    }

    let final_url = response.url().to_string();
    if let Some(tt_secuid) = extract_toutiao_secuid_from_url(&final_url) {
        return Ok(ToutiaoSecuidResult {
            tt_uid: trimmed_uid,
            tt_secuid,
            status: "ok".to_string(),
            error: None,
        });
    }

    let body = response
        .text()
        .map_err(|error| format!("toutiao_secuid_read_failed: {error}"))?;
    let canonical_href = extract_canonical_href(&body).or_else(|| extract_og_url(&body));
    let Some(canonical_href) = canonical_href else {
        return Ok(ToutiaoSecuidResult {
            tt_uid: trimmed_uid,
            tt_secuid: String::new(),
            status: "parse_error".to_string(),
            error: Some(format!("canonical_href_not_found:{final_url}")),
        });
    };

    let tt_secuid = extract_toutiao_secuid_from_url(&canonical_href).unwrap_or_default();
    if tt_secuid.is_empty() {
        return Ok(ToutiaoSecuidResult {
            tt_uid: trimmed_uid,
            tt_secuid,
            status: "parse_error".to_string(),
            error: Some(format!("tt_secuid_not_found:{canonical_href}")),
        });
    }

    Ok(ToutiaoSecuidResult {
        tt_uid: trimmed_uid,
        tt_secuid,
        status: "ok".to_string(),
        error: None,
    })
}

#[tauri::command]
async fn extract_douyin_request_params(
    zip_path: String,
    token_override: Option<String>,
) -> Result<DouyinRequestParamsResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        extract_douyin_request_params_impl(zip_path, token_override)
    })
    .await
    .map_err(|error| format!("task_join_failed: {error}"))?
}

fn extract_douyin_request_params_impl(
    zip_path: String,
    token_override: Option<String>,
) -> Result<DouyinRequestParamsResult, String> {
    let plist_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Preferences/com.ss.iphone.ugc.Aweme.plist"],
    )?
    .ok_or_else(|| "douyin_request_params_failed: preferences file not found".to_string())?;
    let cookie_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Cookies/Cookies.binarycookies"],
    )?;

    let plist_bytes = read_zip_entry_bytes(&zip_path, &plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|error| format!("douyin_request_params_failed: {error}"))?;
    let source = serde_json::to_value(plist_value)
        .map_err(|error| format!("douyin_request_params_failed: {error}"))?;

    let cookie_header = if let Some(path) = cookie_path.as_ref() {
        let cookie_bytes = read_zip_entry_bytes(&zip_path, path)?;
        parse_binarycookies_bytes(&cookie_bytes)?
            .get("cookieHeader")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        String::new()
    };

    let ticket_guard_info = douyin_ticket_guard_info(&source);
    let x_common_params_v2 = douyin_resolve_request_param(
        &source,
        &cookie_header,
        &ticket_guard_info,
        &[
            ParamCandidate::Path(vec!["x-common-params-v2"]),
            ParamCandidate::Computed(douyin_build_common_params_v2(&source, &cookie_header)),
        ],
    );

    let mut headers = BTreeMap::new();
    let mut header_lines = Vec::new();
    let mut push_header = |name: &str, value: Option<String>| {
        if let Some(value) = value {
            if !value.is_empty() {
                header_lines.push(format!("{name}={value}"));
                headers.insert(name.to_string(), value);
            }
        }
    };

    push_header(
        "x-tt-token",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Computed(token_override.clone()),
                ParamCandidate::Path(vec!["kTTAccountTokenGuardXTTToken"]),
                ParamCandidate::Path(vec!["bdaccount_session_x_tt_token"]),
            ],
        ),
    );
    push_header(
        "x-vc-bdturing-sdk-version",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-vc-bdturing-sdk-version"])],
        ),
    );
    push_header(
        "bd-ticket-guard-ree-public-key",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["kBDTGCachedPublicKeyree"])],
        ),
    );
    push_header(
        "x-tt-passport-mfa-token",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["bdaccount_passport_mfa_token"]),
                ParamCandidate::Computed(douyin_cookie_value(&cookie_header, "passport_mfa_token")),
            ],
        ),
    );
    push_header(
        "bd-ticket-guard-client-cert",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["kBDTGClientCertStorageKey"])],
        ),
    );
    push_header(
        "x-tt-token-supplement",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec![
                "accountsdk_extra_headers",
                "x-tt-token-supplement",
            ])],
        ),
    );
    push_header(
        "session-tlb-tag",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["accountsdk_extra_headers", "session-tlb-tag"]),
                ParamCandidate::Computed(douyin_cookie_value(&cookie_header, "session_tlb_tag")),
            ],
        ),
    );
    push_header(
        "x-tt-dt",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-tt-dt"])],
        ),
    );
    push_header(
        "passport-sdk-version",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["passport-sdk-version"]),
                ParamCandidate::Path(vec!["kTTInstallAppVersion"]),
            ],
        ),
    );
    push_header(
        "bd-ticket-guard-version",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["bd-ticket-guard-version"]),
                ParamCandidate::Path(vec!["gurd_kit_app_version"]),
            ],
        ),
    );
    push_header(
        "bd-ticket-guard-key-sign",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["accountsdk_extra_headers", "bd-ticket-guard-key-sign"]),
                ParamCandidate::Computed(douyin_json_string(&ticket_guard_info, "ts_sign")),
            ],
        ),
    );
    push_header(
        "bd-ticket-guard-client-data",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec![
                "kBDTGCreatePrivateKeyLogJSONKeyPrefixtee",
            ])],
        ),
    );
    push_header(
        "passport-sdk-settings",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["kIESAccountSettingsCacheKey"])],
        ),
    );
    push_header(
        "token-tlb-tag",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec![
                "accountsdk_extra_headers",
                "token-tlb-tag",
            ])],
        ),
    );
    push_header(
        "bd-ticket-guard-iteration-version",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec![
                "bd-ticket-guard-iteration-version",
            ])],
        ),
    );
    push_header(
        "sdk-version",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["gurd_kit_app_version"]),
                ParamCandidate::Path(vec!["kTTInstallAppVersion"]),
                ParamCandidate::Path(vec!["bdaccount_x_tt_token_app_version"]),
            ],
        ),
    );
    push_header(
        "x-is-hit-mate",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-is-hit-mate"])],
        ),
    );
    push_header(
        "bd-ticket-guard-static-sign",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["bd-ticket-guard-static-sign"]),
                ParamCandidate::Computed(douyin_json_string(&ticket_guard_info, "ts_sign")),
            ],
        ),
    );
    push_header(
        "bd-ticket-guard-static-ts-sign",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["bd-ticket-guard-static-ts-sign"]),
                ParamCandidate::Computed(douyin_json_string(&ticket_guard_info, "ts_sign_ree")),
            ],
        ),
    );
    push_header(
        "bd-ticket-guard-sec-ts",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[
                ParamCandidate::Path(vec!["bd-ticket-guard-sec-ts"]),
                ParamCandidate::Computed(douyin_json_string(&ticket_guard_info, "ticket")),
            ],
        ),
    );
    push_header(
        "x-tt-store-region",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Computed(douyin_store_region(
                &source,
                &cookie_header,
            ))],
        ),
    );
    push_header(
        "x-tt-store-region-src",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Computed(douyin_cookie_value(
                &cookie_header,
                "store-region-src",
            ))],
        ),
    );
    push_header(
        "x-bd-kmsv",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-bd-kmsv"])],
        ),
    );
    push_header(
        "x-tt-request-tag",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-tt-request-tag"])],
        ),
    );
    push_header(
        "x-tt-ttnet-origin-host",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-tt-ttnet-origin-host"])],
        ),
    );
    push_header(
        "x-ss-dp",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["kBDUGPushSDKAID"])],
        ),
    );
    push_header(
        "x-tt-trace-id",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-tt-trace-id"])],
        ),
    );
    push_header(
        "ttzip-version",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["ttzip-version"])],
        ),
    );
    push_header("cookie", Some(cookie_header.clone()));
    push_header(
        "x-argus",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-argus"])],
        ),
    );
    push_header(
        "x-gorgon",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-gorgon"])],
        ),
    );
    push_header(
        "x-helios",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-helios"])],
        ),
    );
    push_header(
        "x-khronos",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-khronos"])],
        ),
    );
    push_header(
        "x-ladon",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-ladon"])],
        ),
    );
    push_header(
        "x-medusa",
        douyin_resolve_request_param(
            &source,
            &cookie_header,
            &ticket_guard_info,
            &[ParamCandidate::Path(vec!["x-medusa"])],
        ),
    );
    push_header("x-common-params-v2", x_common_params_v2.clone());

    let header_text = header_lines.join("\n");
    let sec_user_id = x_common_params_v2
        .as_ref()
        .and_then(|value| extract_query_param(value, "sec_user_id"))
        .unwrap_or_else(|| douyin_sec_user_id(&source).unwrap_or_default());

    Ok(DouyinRequestParamsResult {
        source_zip: zip_path,
        source_plist_path: plist_path,
        source_cookie_path: cookie_path,
        sec_user_id,
        cookie_header,
        header_count: headers.len(),
        header_text,
        headers,
    })
}

#[tauri::command]
async fn check_douyin_password_status(
    zip_path: String,
    session_id_override: Option<String>,
) -> Result<DouyinPasswordStatusResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_douyin_password_status_impl(zip_path, session_id_override)
    })
    .await
    .map_err(|error| format!("task_join_failed: {error}"))?
}

fn check_douyin_password_status_impl(
    zip_path: String,
    session_id_override: Option<String>,
) -> Result<DouyinPasswordStatusResult, String> {
    let local_password_status = read_douyin_local_account_payload(&zip_path)?
        .map(|payload| parse_douyin_password_status_payload(&payload))
        .unwrap_or_default();
    let has_local_status = has_douyin_password_status_data(&local_password_status);

    let cookie_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Cookies/Cookies.binarycookies"],
    )?;

    let session_id = if let Some(override_id) = session_id_override.filter(|s| !s.is_empty()) {
        override_id
    } else {
        let Some(cookie_path) = &cookie_path else {
            if has_local_status {
                return Ok(parsed_douyin_password_status_to_result(
                    zip_path,
                    None,
                    String::new(),
                    local_password_status,
                    None,
                ));
            }
            return Ok(DouyinPasswordStatusResult {
                source_zip: zip_path,
                source_cookie_path: None,
                session_id: String::new(),
                has_password: None,
                account_name: None,
                register_time: None,
                bindings: DouyinSessionBindings::default(),
                status: "missing_cookie".to_string(),
                error: Some("douyin_cookie_file_not_found".to_string()),
            });
        };

        let cookie_bytes = read_zip_entry_bytes(&zip_path, cookie_path)?;
        let parsed_cookies = parse_binarycookies_bytes(&cookie_bytes)?;
        let cookie_header = parsed_cookies
            .get("cookieHeader")
            .and_then(Value::as_str)
            .unwrap_or_default();
        extract_douyin_session_id(cookie_header).unwrap_or_default()
    };

    if session_id.is_empty() {
        let has_local_status = has_douyin_password_status_data(&local_password_status);

        if has_local_status {
            return Ok(parsed_douyin_password_status_to_result(
                zip_path,
                cookie_path,
                session_id,
                local_password_status,
                Some("douyin_sessionid_not_found".to_string()),
            ));
        }
        return Ok(DouyinPasswordStatusResult {
            source_zip: zip_path,
            source_cookie_path: cookie_path,
            session_id,
            has_password: None,
            account_name: None,
            register_time: None,
            bindings: DouyinSessionBindings::default(),
            status: "missing_sessionid".to_string(),
            error: Some("douyin_sessionid_not_found".to_string()),
        });
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("douyin_password_status_client_failed: {error}"))?;
    let response = client
        .get("https://api5-normal-lf.amemv.com/passport/account/info/v2/?is_from_iesaccountsaas=1&verify_sdk_version=4.1.0&is_from_ttaccountsdk=1&passport_support_flow=verify,real_name_check&auth_sdk_version=5.0.4.1&passport-sdk-version=7.2.12-alpha.53&identity_token_client_request=1&multi_login=1&multi_login=1&resolution=828*1792&app_id=1128&sec_sdk_version=67764225&ttnet_sdk_version=4.2.243.21-douyin&install_id=1417749611157356&account_app_language=zh&ssmix=a&in_sp_time=1&lite_user_dependency=1&user_api_need_combine=1&ttnet_version=4.2.243.21-douyin&use_store_region_cookie=1")
        .header("x-ss-cookie", format!("sessionid={session_id}"))
        .send();
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            if has_local_status {
                return Ok(parsed_douyin_password_status_to_result(
                    zip_path,
                    cookie_path,
                    session_id,
                    local_password_status,
                    Some(format!("douyin_password_status_request_failed: {error}")),
                ));
            }
            return Err(format!("douyin_password_status_request_failed: {error}"));
        }
    };

    let status_code = response.status();
    if !status_code.is_success() {
        if has_local_status {
            return Ok(parsed_douyin_password_status_to_result(
                zip_path,
                cookie_path,
                session_id,
                local_password_status,
                Some(format!("http_status_{status_code}")),
            ));
        }
        return Ok(DouyinPasswordStatusResult {
            source_zip: zip_path,
            source_cookie_path: cookie_path,
            session_id,
            has_password: None,
            account_name: None,
            register_time: None,
            bindings: DouyinSessionBindings::default(),
            status: "http_error".to_string(),
            error: Some(format!("http_status_{status_code}")),
        });
    }

    let payload = response.json::<Value>();
    let payload = match payload {
        Ok(payload) => payload,
        Err(error) => {
            if has_local_status {
                return Ok(parsed_douyin_password_status_to_result(
                    zip_path,
                    cookie_path,
                    session_id,
                    local_password_status,
                    Some(format!("douyin_password_status_decode_failed: {error}")),
                ));
            }
            return Err(format!("douyin_password_status_decode_failed: {error}"));
        }
    };
    let parsed = merge_douyin_password_status(
        local_password_status,
        Some(parse_douyin_password_status_payload(&payload)),
    );

    Ok(parsed_douyin_password_status_to_result(
        zip_path,
        cookie_path,
        session_id,
        parsed,
        None,
    ))
}

#[tauri::command]
async fn check_douyin_certification_status(
    zip_path: String,
) -> Result<DouyinCertificationStatusResult, String> {
    tauri::async_runtime::spawn_blocking(move || check_douyin_certification_status_impl(zip_path))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

fn check_douyin_certification_status_impl(
    zip_path: String,
) -> Result<DouyinCertificationStatusResult, String> {
    let plist_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Preferences/com.ss.iphone.ugc.Aweme.plist"],
    )?;
    let local_payload = read_douyin_local_account_payload(&zip_path)?;

    let Some(payload) = local_payload else {
        return Ok(DouyinCertificationStatusResult {
            source_zip: zip_path,
            source_plist_path: plist_path.clone(),
            is_verified: None,
            account_name: None,
            status: if plist_path.is_some() {
                "missing_local_account".to_string()
            } else {
                "missing_plist".to_string()
            },
            error: Some(if plist_path.is_some() {
                "douyin_local_account_not_found".to_string()
            } else {
                "douyin_preferences_file_not_found".to_string()
            }),
        });
    };

    let parsed = parse_douyin_certification_status_payload(&payload);

    Ok(DouyinCertificationStatusResult {
        source_zip: zip_path,
        source_plist_path: plist_path,
        is_verified: parsed.is_verified,
        account_name: parsed.screen_name,
        status: match parsed.is_verified {
            Some(true) => "ok".to_string(),
            Some(false) => "not_verified".to_string(),
            None => "parse_error".to_string(),
        },
        error: match parsed.is_verified {
            Some(_) => None,
            None => Some("douyin_is_verified_not_found".to_string()),
        },
    })
}

#[tauri::command]
async fn check_douyin_token_status(
    zip_path: String,
    token_override: Option<String>,
) -> Result<DouyinTokenStatusResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_douyin_token_status_impl(zip_path, token_override)
    })
    .await
    .map_err(|error| format!("task_join_failed: {error}"))?
}

fn check_douyin_token_status_impl(
    zip_path: String,
    token_override: Option<String>,
) -> Result<DouyinTokenStatusResult, String> {
    let local_phone_number = read_douyin_local_phone_number(&zip_path).ok().flatten();
    let plist_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Preferences/com.ss.iphone.ugc.Aweme.plist"],
    )?;
    let cookie_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Cookies/Cookies.binarycookies"],
    )?;

    let Some(plist_path) = plist_path else {
        return Ok(DouyinTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: None,
            source_cookie_path: cookie_path,
            token_preview: String::new(),
            odin_tt_preview: String::new(),
            local_phone_number,
            status: "missing_plist".to_string(),
            valid_endpoint_count: 0,
            endpoints: Vec::new(),
            functions: Vec::new(),
            error: Some("douyin_preferences_file_not_found".to_string()),
        });
    };

    let plist_bytes = read_zip_entry_bytes(&zip_path, &plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|error| format!("douyin_token_status_failed: {error}"))?;
    let source = serde_json::to_value(plist_value)
        .map_err(|error| format!("douyin_token_status_failed: {error}"))?;

    let x_tt_token = if let Some(override_token) = token_override.filter(|t| !t.is_empty()) {
        override_token
    } else {
        douyin_token_value(&source)
    };
    if x_tt_token.is_empty() {
        return Ok(DouyinTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: cookie_path,
            token_preview: String::new(),
            odin_tt_preview: String::new(),
            local_phone_number,
            status: "missing_token".to_string(),
            valid_endpoint_count: 0,
            endpoints: Vec::new(),
            functions: Vec::new(),
            error: Some("douyin_x_tt_token_not_found".to_string()),
        });
    }

    if is_douyin_act_token(&x_tt_token) {
        return Ok(DouyinTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: cookie_path,
            token_preview: mask_secret(&x_tt_token),
            odin_tt_preview: String::new(),
            local_phone_number,
            status: "skipped_act_token".to_string(),
            valid_endpoint_count: 0,
            endpoints: Vec::new(),
            functions: Vec::new(),
            error: None,
        });
    }

    let Some(cookie_path) = cookie_path else {
        return Ok(DouyinTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: None,
            token_preview: mask_secret(&x_tt_token),
            odin_tt_preview: String::new(),
            local_phone_number,
            status: "missing_cookie".to_string(),
            valid_endpoint_count: 0,
            endpoints: Vec::new(),
            functions: Vec::new(),
            error: Some("douyin_cookie_file_not_found".to_string()),
        });
    };

    let cookie_bytes = read_zip_entry_bytes(&zip_path, &cookie_path)?;
    let parsed_cookies = parse_binarycookies_bytes(&cookie_bytes)?;
    let cookie_header = parsed_cookies
        .get("cookieHeader")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let odin_tt = extract_cookie_value(cookie_header, "odin_tt").unwrap_or_default();
    if odin_tt.is_empty() {
        return Ok(DouyinTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: Some(cookie_path),
            token_preview: mask_secret(&x_tt_token),
            odin_tt_preview: String::new(),
            local_phone_number,
            status: "missing_odin_tt".to_string(),
            valid_endpoint_count: 0,
            endpoints: Vec::new(),
            functions: Vec::new(),
            error: Some("douyin_odin_tt_not_found".to_string()),
        });
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("douyin_token_status_client_failed: {error}"))?;
    let mut endpoints = [
        DouyinTokenEndpoint::SafetyPortrait,
        DouyinTokenEndpoint::ProfileSelf,
    ]
    .into_iter()
    .map(|endpoint| {
        request_douyin_token_endpoint(
            &client,
            endpoint,
            &source,
            cookie_header,
            &x_tt_token,
            &odin_tt,
        )
    })
    .collect::<Vec<_>>();
    if let Some(phone_number) = local_phone_number
        .clone()
        .filter(|value| !value.trim().is_empty())
    {
        if endpoints.iter().all(|endpoint| {
            endpoint
                .phone_number
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        }) {
            if let Some(endpoint) = endpoints.first_mut() {
                endpoint.phone_number = Some(phone_number);
            }
        }
    }
    let valid_endpoint_count = endpoints
        .iter()
        .filter(|endpoint| endpoint.status == "ok")
        .count();
    let status = if valid_endpoint_count > 0 {
        "ok".to_string()
    } else if endpoints
        .iter()
        .any(|endpoint| endpoint.status == "invalid")
    {
        "invalid".to_string()
    } else if endpoints
        .iter()
        .any(|endpoint| endpoint.status == "http_error")
    {
        "http_error".to_string()
    } else if endpoints
        .iter()
        .any(|endpoint| endpoint.status == "request_error")
    {
        "request_error".to_string()
    } else {
        "parse_error".to_string()
    };
    let error = if valid_endpoint_count > 0 {
        None
    } else {
        endpoints
            .iter()
            .find_map(|endpoint| endpoint.message.clone())
            .or_else(|| Some("douyin_token_check_not_validated".to_string()))
    };

    let functions = endpoints.iter().flat_map(|e| e.functions.clone()).collect();

    Ok(DouyinTokenStatusResult {
        source_zip: zip_path,
        source_plist_path: Some(plist_path),
        source_cookie_path: Some(cookie_path),
        token_preview: mask_secret(&x_tt_token),
        odin_tt_preview: mask_secret(&odin_tt),
        local_phone_number,
        status,
        valid_endpoint_count,
        endpoints,
        functions,
        error,
    })
}

#[tauri::command]
async fn extract_douyin_account_credentials(
    zip_path: String,
) -> Result<DouyinAccountCredentialResult, String> {
    tauri::async_runtime::spawn_blocking(move || extract_douyin_account_credentials_impl(zip_path))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

fn extract_douyin_account_credentials_impl(
    zip_path: String,
) -> Result<DouyinAccountCredentialResult, String> {
    let plist_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Preferences/com.ss.iphone.ugc.Aweme.plist"],
    )?;
    let cookie_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Cookies/Cookies.binarycookies"],
    )?;

    let Some(plist_path) = plist_path else {
        return Ok(DouyinAccountCredentialResult {
            source_zip: zip_path,
            source_plist_path: None,
            source_cookie_path: cookie_path,
            current_session_id_preview: String::new(),
            current_token_preview: String::new(),
            current_odin_tt_preview: String::new(),
            account_count: 0,
            accounts: Vec::new(),
            status: "missing_plist".to_string(),
            error: Some("douyin_preferences_file_not_found".to_string()),
        });
    };

    let plist_bytes = read_zip_entry_bytes(&zip_path, &plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|error| format!("douyin_account_credentials_failed: {error}"))?;
    let source = serde_json::to_value(plist_value.clone())
        .map_err(|error| format!("douyin_account_credentials_failed: {error}"))?;

    let current_token = douyin_token_value(&source);
    let session_map = parse_douyin_multi_session_map(
        source
            .get("com.toutiao.account.userdefault.user.mutil_sids_v2")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let token_map = extract_douyin_token_cluster_map(&plist_bytes);
    let mmkv_default_bytes = read_douyin_mmkv_default_bytes(&zip_path).ok().flatten();
    let accountsaaskit_sec_uid_map = read_douyin_accountsaaskit_bytes(&zip_path)
        .ok()
        .flatten()
        .map(|bytes| extract_uid_sec_uid_pairs_from_accountsaaskit(&bytes))
        .unwrap_or_default();
    let all_payload = read_douyin_local_account_payload_from_plist_value(
        &plist_value,
        "kDYAAllLoginUserPersistenceKey",
    )?;
    let current_payload = read_douyin_local_account_payload_from_plist_value(
        &plist_value,
        "kDYACurrentLoginUserPersistenceKey",
    )?;
    let current_identity = current_payload
        .as_ref()
        .and_then(parse_douyin_local_account_identity_from_payload);
    let mut local_accounts = all_payload
        .as_ref()
        .map(parse_douyin_local_account_identities)
        .unwrap_or_default();

    // Merge third-party platform connects from local SDK user info archive
    // (QQ / Google / WeChat / Apple / Toutiao) as offline fallback
    if let Ok(sdk_connects) = read_douyin_sdk_user_info_connects(&zip_path) {
        for (uid, connect) in &sdk_connects {
            let connect_bindings = parse_douyin_session_bindings_for_uid(connect, Some(uid));
            if let Some(local) = local_accounts.iter_mut().find(|item| &item.uid == uid) {
                local.bindings = merge_douyin_session_bindings(
                    std::mem::take(&mut local.bindings),
                    connect_bindings,
                );
            } else {
                // Account not yet in local_accounts (e.g. incomplete plist data).
                // Create a minimal entry carrying the SDK-provided bindings.
                local_accounts.push(DouyinLocalAccountIdentity {
                    uid: uid.clone(),
                    bindings: connect_bindings,
                    nickname: String::new(),
                    sec_uid: String::new(),
                    unique_id: String::new(),
                    short_id: String::new(),
                    phone_number: String::new(),
                    register_time: String::new(),
                    aweme_count: String::new(),
                    following_count: String::new(),
                    liked_count: String::new(),
                    has_password: None,
                    is_verified: None,
                    normal_functions: Vec::new(),
                });
            }
        }
    }

    let mut current_session = source
        .get("com.toutiao.account.userdefault.sessionid")
        .and_then(douyin_normalize_json_value)
        .unwrap_or_default();
    let mut current_odin_tt = String::new();
    if let Some(cookie_path) = cookie_path.as_ref() {
        let cookie_bytes = read_zip_entry_bytes(&zip_path, cookie_path)?;
        let parsed_cookies = parse_binarycookies_bytes(&cookie_bytes)?;
        let cookie_header = parsed_cookies
            .get("cookieHeader")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(value) = extract_douyin_session_id(cookie_header) {
            current_session = value;
        }
        current_odin_tt = extract_cookie_value(cookie_header, "odin_tt").unwrap_or_default();
    }

    let mut ordered_uids = Vec::new();
    let mut known = BTreeMap::new();
    for account in &local_accounts {
        if account.uid.is_empty() || known.contains_key(&account.uid) {
            continue;
        }
        known.insert(account.uid.clone(), true);
        ordered_uids.push(account.uid.clone());
    }
    for uid in session_map.keys() {
        if !known.contains_key(uid) {
            known.insert(uid.clone(), true);
            ordered_uids.push(uid.clone());
        }
    }
    for uid in token_map.keys() {
        if !known.contains_key(uid) {
            known.insert(uid.clone(), true);
            ordered_uids.push(uid.clone());
        }
    }

    let mut accounts = Vec::new();
    for uid in ordered_uids {
        let local = local_accounts.iter().find(|item| item.uid == uid);
        let preferred_local = current_identity
            .as_ref()
            .filter(|identity| identity.uid == uid);
        let merged_local = merge_douyin_local_account_identity(local, preferred_local);
        let token_entry = token_map.get(&uid);
        let session_value = session_map
            .get(&uid)
            .cloned()
            .or_else(|| {
                current_identity
                    .as_ref()
                    .filter(|identity| identity.uid == uid)
                    .map(|_| current_session.clone())
            })
            .unwrap_or_default();
        let access_token = token_entry
            .map(|entry| entry.access_token.clone())
            .or_else(|| {
                current_identity
                    .as_ref()
                    .filter(|identity| identity.uid == uid)
                    .map(|_| current_token.clone())
            })
            .unwrap_or_default();
        let open_id = token_entry
            .map(|entry| entry.open_id.clone())
            .unwrap_or_default();
        let merged_sec_uid = first_non_empty_strings(&[
            Some(merged_local.sec_uid.clone()),
            accountsaaskit_sec_uid_map
                .get(&uid)
                .cloned()
                .filter(|value| !value.trim().is_empty()),
            token_entry
                .map(|entry| entry.sec_uid.clone())
                .filter(|value| !value.trim().is_empty()),
        ])
        .unwrap_or_default();
        let merged_unique_id = first_non_empty_strings(&[
            Some(merged_local.unique_id.clone()),
            mmkv_default_bytes
                .as_deref()
                .and_then(|bytes| extract_unique_id_near_sec_uid(bytes, &merged_sec_uid)),
        ])
        .unwrap_or_default();
        let is_current = preferred_local.is_some()
            || (!current_session.is_empty() && session_value == current_session);

        accounts.push(DouyinAccountCredentialItem {
            uid: uid.clone(),
            nickname: merged_local.nickname.clone(),
            sec_uid: merged_sec_uid,
            unique_id: merged_unique_id,
            short_id: merged_local.short_id.clone(),
            session_id: session_value.clone(),
            session_id_preview: mask_secret(&session_value),
            access_token: access_token.clone(),
            access_token_preview: mask_secret(&access_token),
            open_id: open_id.clone(),
            open_id_preview: token_entry
                .map(|entry| mask_secret(&entry.open_id))
                .unwrap_or_default(),
            auth_time_label: token_entry
                .map(|entry| entry.auth_time_label.clone())
                .unwrap_or_default(),
            is_current,
            phone_number: merged_local.phone_number,
            register_time: merged_local.register_time,
            aweme_count: merged_local.aweme_count,
            following_count: merged_local.following_count,
            liked_count: merged_local.liked_count,
            bindings: merged_local.bindings,
            has_password: merged_local.has_password,
            is_verified: merged_local.is_verified,
            normal_functions: merged_local.normal_functions,
        });
    }

    let account_count = accounts.len();
    let status = if account_count > 0 {
        "ok".to_string()
    } else if !session_map.is_empty() || !token_map.is_empty() {
        "partial".to_string()
    } else {
        "missing_accounts".to_string()
    };

    Ok(DouyinAccountCredentialResult {
        source_zip: zip_path,
        source_plist_path: Some(plist_path),
        source_cookie_path: cookie_path,
        current_session_id_preview: mask_secret(&current_session),
        current_token_preview: mask_secret(&current_token),
        current_odin_tt_preview: mask_secret(&current_odin_tt),
        account_count,
        accounts,
        status: status.clone(),
        error: if status == "missing_accounts" {
            Some("douyin_multi_account_credentials_not_found".to_string())
        } else {
            None
        },
    })
}

#[tauri::command]
async fn check_toutiao_token_status(zip_path: String) -> Result<ToutiaoTokenStatusResult, String> {
    tauri::async_runtime::spawn_blocking(move || check_toutiao_token_status_impl(zip_path))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

fn check_toutiao_token_status_impl(zip_path: String) -> Result<ToutiaoTokenStatusResult, String> {
    let plist_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.article.News",
        &["Library/Preferences/com.ss.iphone.article.news.plist"],
    )?;
    let cookie_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.article.News",
        &["Library/Cookies/Cookies.binarycookies"],
    )?;

    let Some(plist_path) = plist_path else {
        return Ok(ToutiaoTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: None,
            source_cookie_path: cookie_path,
            token_preview: String::new(),
            odin_tt_preview: String::new(),
            device_id: String::new(),
            iid: String::new(),
            nickname: None,
            uid: None,
            register_time: None,
            http_status: None,
            status: "missing_plist".to_string(),
            error: Some("toutiao_preferences_file_not_found".to_string()),
        });
    };
    let Some(cookie_path) = cookie_path else {
        return Ok(ToutiaoTokenStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: None,
            token_preview: String::new(),
            odin_tt_preview: String::new(),
            device_id: String::new(),
            iid: String::new(),
            nickname: None,
            uid: None,
            register_time: None,
            http_status: None,
            status: "missing_cookie".to_string(),
            error: Some("toutiao_cookie_file_not_found".to_string()),
        });
    };

    let plist_bytes = read_zip_entry_bytes(&zip_path, &plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|_| "toutiao_token_plist_decode_failed".to_string())?;
    let source = serde_json::to_value(plist_value)
        .map_err(|_| "toutiao_token_plist_convert_failed".to_string())?;
    let token = toutiao_token_value(&source);
    let device_id = toutiao_device_id(&source);

    let cookie_bytes = read_zip_entry_bytes(&zip_path, &cookie_path)?;
    let parsed_cookies = parse_binarycookies_bytes(&cookie_bytes)?;
    let odin_tt = toutiao_cookie_value(&parsed_cookies, "odin_tt").unwrap_or_default();
    let iid = toutiao_cookie_value(&parsed_cookies, "install_id").unwrap_or_default();

    let base_result = |status: &str, error: &str| ToutiaoTokenStatusResult {
        source_zip: zip_path.clone(),
        source_plist_path: Some(plist_path.clone()),
        source_cookie_path: Some(cookie_path.clone()),
        token_preview: mask_secret(&token),
        odin_tt_preview: mask_secret(&odin_tt),
        device_id: device_id.clone(),
        iid: iid.clone(),
        nickname: None,
        uid: None,
        register_time: None,
        http_status: None,
        status: status.to_string(),
        error: Some(error.to_string()),
    };

    if token.is_empty() {
        return Ok(base_result("missing_token", "toutiao_token_not_found"));
    }
    if odin_tt.is_empty() {
        return Ok(base_result("missing_odin_tt", "toutiao_odin_tt_not_found"));
    }
    if device_id.is_empty() {
        return Ok(base_result(
            "missing_device_id",
            "toutiao_device_id_not_found",
        ));
    }
    if iid.is_empty() {
        return Ok(base_result("missing_iid", "toutiao_install_id_not_found"));
    }

    let cookie_header = [
        ("odin_tt", Some(odin_tt.clone())),
        (
            "store-region",
            toutiao_cookie_value(&parsed_cookies, "store-region"),
        ),
        (
            "store-region-src",
            toutiao_cookie_value(&parsed_cookies, "store-region-src"),
        ),
        (
            "passport_csrf_token",
            toutiao_cookie_value(&parsed_cookies, "passport_csrf_token"),
        ),
        (
            "passport_csrf_token_default",
            toutiao_cookie_value(&parsed_cookies, "passport_csrf_token_default"),
        ),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.map(|value| format!("{name}={value}")))
    .collect::<Vec<_>>()
    .join(";");
    let app_version = source
        .get("kTTInstallAppVersion")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "16.4.0".to_string());
    let launch_version = source
        .get("kUserDefaultsLaunchVersionkey")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{app_version}.20"));
    let user_agent =
        format!("News {app_version} rv:{launch_version} (iPhone; iOS 16.1.1; zh-Hans_HK) Cronet");

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|_| "toutiao_token_client_failed".to_string())?;
    let response = match client
        .get("https://api5-normal-hl.toutiaoapi.com/tabs_api/v1/")
        .query(&[
            ("app_name", "news_article"),
            ("device_id", device_id.as_str()),
            ("aid", "13"),
            ("iid", iid.as_str()),
            ("detail", "my_tabs_v2"),
            ("user_app_id", "1128"),
        ])
        .header("Accept", "*/*")
        .header("Cookie", cookie_header)
        .header("User-Agent", user_agent)
        .header("sdk-version", "2")
        .header("x-tt-token", &token)
        .send()
    {
        Ok(response) => response,
        Err(_) => {
            return Ok(base_result("request_error", "toutiao_token_request_failed"));
        }
    };
    let http_status = response.status();
    if !http_status.is_success() {
        let mut result = base_result("http_error", "toutiao_token_http_error");
        result.http_status = Some(http_status.as_u16());
        return Ok(result);
    }
    let payload = match response.json::<Value>() {
        Ok(payload) => payload,
        Err(_) => {
            let mut result = base_result("parse_error", "toutiao_token_response_decode_failed");
            result.http_status = Some(http_status.as_u16());
            return Ok(result);
        }
    };
    let parsed = parse_toutiao_token_payload(&payload);
    let status = match parsed.is_valid {
        Some(true) => "ok",
        Some(false) => "invalid",
        None => "parse_error",
    };
    let error = match parsed.is_valid {
        Some(true) => None,
        Some(false) => Some(
            parsed
                .message
                .clone()
                .unwrap_or_else(|| "toutiao_token_invalid".to_string()),
        ),
        None => Some("toutiao_token_profile_not_found".to_string()),
    };

    Ok(ToutiaoTokenStatusResult {
        source_zip: zip_path,
        source_plist_path: Some(plist_path),
        source_cookie_path: Some(cookie_path),
        token_preview: mask_secret(&token),
        odin_tt_preview: mask_secret(&odin_tt),
        device_id,
        iid,
        nickname: parsed.nickname,
        uid: parsed.uid,
        register_time: parsed.register_time,
        http_status: Some(http_status.as_u16()),
        status: status.to_string(),
        error,
    })
}

#[tauri::command]
async fn check_toutiao_certification_status(
    zip_path: String,
) -> Result<ToutiaoCertificationStatusResult, String> {
    tauri::async_runtime::spawn_blocking(move || check_toutiao_certification_status_impl(zip_path))
        .await
        .map_err(|error| format!("task_join_failed: {error}"))?
}

fn check_toutiao_certification_status_impl(
    zip_path: String,
) -> Result<ToutiaoCertificationStatusResult, String> {
    let plist_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.article.News",
        &["Library/Preferences/com.ss.iphone.article.news.plist"],
    )?;
    let cookie_path = find_app_file_path(
        &zip_path,
        "com.ss.iphone.article.News",
        &["Library/Cookies/Cookies.binarycookies"],
    )?;

    let Some(plist_path) = plist_path else {
        return Ok(ToutiaoCertificationStatusResult {
            source_zip: zip_path,
            source_plist_path: None,
            source_cookie_path: cookie_path,
            act_token: String::new(),
            odin_tt: String::new(),
            is_verified: None,
            status: "missing_plist".to_string(),
            error: Some("toutiao_preferences_file_not_found".to_string()),
        });
    };
    let Some(cookie_path) = cookie_path else {
        return Ok(ToutiaoCertificationStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: None,
            act_token: String::new(),
            odin_tt: String::new(),
            is_verified: None,
            status: "missing_cookie".to_string(),
            error: Some("toutiao_cookie_file_not_found".to_string()),
        });
    };

    let plist_bytes = read_zip_entry_bytes(&zip_path, &plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|error| format!("toutiao_certification_status_failed: {error}"))?;
    let source = serde_json::to_value(plist_value)
        .map_err(|error| format!("toutiao_certification_status_failed: {error}"))?;
    let act_token = douyin_json_value(&source, &["bdaccount_session_x_tt_token"])
        .and_then(douyin_normalize_json_value)
        .unwrap_or_default();
    if act_token.is_empty() {
        return Ok(ToutiaoCertificationStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: Some(cookie_path),
            act_token,
            odin_tt: String::new(),
            is_verified: None,
            status: "missing_act_token".to_string(),
            error: Some("toutiao_act_token_not_found".to_string()),
        });
    }

    let cookie_bytes = read_zip_entry_bytes(&zip_path, &cookie_path)?;
    let parsed_cookies = parse_binarycookies_bytes(&cookie_bytes)?;
    let cookie_header = parsed_cookies
        .get("cookieHeader")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let odin_tt = extract_cookie_value(cookie_header, "odin_tt").unwrap_or_default();
    if odin_tt.is_empty() {
        return Ok(ToutiaoCertificationStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: Some(cookie_path),
            act_token,
            odin_tt,
            is_verified: None,
            status: "missing_odin_tt".to_string(),
            error: Some("toutiao_odin_tt_not_found".to_string()),
        });
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| format!("toutiao_certification_status_client_failed: {error}"))?;
    let response = client
        .get("https://webcast5-open-lf.douyin.com/webcast/openapi/certification/get_certification_status/?webcast_app_id=6822&aid=13")
        .header("Accept", "*/*")
        .header("Cookie", format!("odin_tt={odin_tt}"))
        .header(
            "User-Agent",
            "News 16.4.0 rv:16.4.0.20 (iPhone; iOS 16.1.1; zh-Hans_HK) Cronet",
        )
        .header("authorization", format!("Bearer {act_token}"))
        .send()
        .map_err(|error| format!("toutiao_certification_status_request_failed: {error}"))?;

    let status_code = response.status();
    if !status_code.is_success() {
        return Ok(ToutiaoCertificationStatusResult {
            source_zip: zip_path,
            source_plist_path: Some(plist_path),
            source_cookie_path: Some(cookie_path),
            act_token,
            odin_tt,
            is_verified: None,
            status: "http_error".to_string(),
            error: Some(format!("http_status_{status_code}")),
        });
    }

    let payload = response
        .json::<Value>()
        .map_err(|error| format!("toutiao_certification_status_decode_failed: {error}"))?;
    let parsed = parse_toutiao_certification_status_payload(&payload);

    Ok(ToutiaoCertificationStatusResult {
        source_zip: zip_path,
        source_plist_path: Some(plist_path),
        source_cookie_path: Some(cookie_path),
        act_token,
        odin_tt,
        is_verified: parsed.is_verified,
        status: match parsed.is_verified {
            Some(true) => "ok".to_string(),
            Some(false) => "not_verified".to_string(),
            None => "parse_error".to_string(),
        },
        error: match parsed.is_verified {
            Some(_) => None,
            None => Some("toutiao_is_verified_not_found".to_string()),
        },
    })
}

fn parse_json_file(
    zip_path: &str,
    inner_path: &str,
    app_id: &str,
    file_type: &str,
) -> Result<ParseResult, String> {
    let bytes = read_zip_entry_bytes(zip_path, inner_path)?;
    let text = String::from_utf8_lossy(&bytes).to_string();

    match serde_json::from_str::<Value>(&text) {
        Ok(parsed_data) => Ok(ParseResult {
            source_zip: zip_path.to_string(),
            app_id: app_id.to_string(),
            inner_path: inner_path.to_string(),
            file_type: file_type.to_string(),
            parse_status: "ok".to_string(),
            parsed_data,
            meta: json!({ "byteLength": bytes.len() }),
            error: None,
        }),
        Err(error) => Ok(ParseResult {
            source_zip: zip_path.to_string(),
            app_id: app_id.to_string(),
            inner_path: inner_path.to_string(),
            file_type: file_type.to_string(),
            parse_status: "error".to_string(),
            parsed_data: json!({ "rawText": preview_string(&text, 4000) }),
            meta: json!({ "byteLength": bytes.len() }),
            error: Some(format!("json_parse_failed: {error}")),
        }),
    }
}

fn parse_plist_file(
    zip_path: &str,
    inner_path: &str,
    app_id: &str,
    file_type: &str,
) -> Result<ParseResult, String> {
    let bytes = read_zip_entry_bytes(zip_path, inner_path)?;

    match plist::Value::from_reader(Cursor::new(bytes.as_slice())) {
        Ok(plist_value) => {
            let parsed_data = serde_json::to_value(plist_value)
                .map_err(|error| format!("plist_parse_failed: {error}"))?;

            Ok(ParseResult {
                source_zip: zip_path.to_string(),
                app_id: app_id.to_string(),
                inner_path: inner_path.to_string(),
                file_type: file_type.to_string(),
                parse_status: "ok".to_string(),
                parsed_data,
                meta: json!({ "byteLength": bytes.len() }),
                error: None,
            })
        }
        Err(error) => Ok(ParseResult {
            source_zip: zip_path.to_string(),
            app_id: app_id.to_string(),
            inner_path: inner_path.to_string(),
            file_type: file_type.to_string(),
            parse_status: "error".to_string(),
            parsed_data: Value::Null,
            meta: json!({ "byteLength": bytes.len() }),
            error: Some(format!("plist_parse_failed: {error}")),
        }),
    }
}

fn parse_sqlite_file(
    zip_path: &str,
    inner_path: &str,
    app_id: &str,
    file_type: &str,
) -> Result<ParseResult, String> {
    let temp_dir = tempdir().map_err(|error| format!("sqlite_extract_failed: {error}"))?;
    let file_name = Path::new(inner_path)
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "invalid_path_layout".to_string())?;
    let db_path = temp_dir.path().join(file_name);

    if !extract_entry_to_path_from_source(zip_path, inner_path, &db_path)? {
        return Err("sqlite_extract_failed: missing database file".to_string());
    }

    let wal_inner_path = format!("{inner_path}-wal");
    let shm_inner_path = format!("{inner_path}-shm");
    let wal_path = temp_dir.path().join(format!("{file_name}-wal"));
    let shm_path = temp_dir.path().join(format!("{file_name}-shm"));

    let includes_wal = extract_entry_to_path_from_source(zip_path, &wal_inner_path, &wal_path)?;
    let includes_shm = extract_entry_to_path_from_source(zip_path, &shm_inner_path, &shm_path)?;

    let connection =
        Connection::open(&db_path).map_err(|error| format!("sqlite_open_failed: {error}"))?;
    let preview = sqlite_preview(&connection)?;

    Ok(ParseResult {
        source_zip: zip_path.to_string(),
        app_id: app_id.to_string(),
        inner_path: inner_path.to_string(),
        file_type: file_type.to_string(),
        parse_status: "ok".to_string(),
        parsed_data: preview,
        meta: json!({
            "includesWal": includes_wal,
            "includesShm": includes_shm,
        }),
        error: None,
    })
}

fn parse_binarycookies_file(
    zip_path: &str,
    inner_path: &str,
    app_id: &str,
    file_type: &str,
) -> Result<ParseResult, String> {
    let bytes = read_zip_entry_bytes(zip_path, inner_path)?;
    let parsed_data = parse_binarycookies_bytes(&bytes)?;

    Ok(ParseResult {
        source_zip: zip_path.to_string(),
        app_id: app_id.to_string(),
        inner_path: inner_path.to_string(),
        file_type: file_type.to_string(),
        parse_status: "ok".to_string(),
        parsed_data,
        meta: json!({ "byteLength": bytes.len() }),
        error: None,
    })
}

fn parse_binarycookies_bytes(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() < 8 {
        return Err("binarycookies_parse_failed: file too small".to_string());
    }

    if &bytes[0..4] != b"cook" {
        return Err("binarycookies_parse_failed: invalid file header".to_string());
    }

    let page_count = usize::try_from(read_u32_be(bytes, 4)?)
        .map_err(|_| "binarycookies_parse_failed: invalid page count".to_string())?;
    let mut offset = 8usize;
    let mut page_sizes = Vec::with_capacity(page_count);

    for _ in 0..page_count {
        let page_size = usize::try_from(read_u32_be(bytes, offset)?)
            .map_err(|_| "binarycookies_parse_failed: invalid page size".to_string())?;
        page_sizes.push(page_size);
        offset += 4;
    }

    let mut cookies = Vec::new();

    for page_size in page_sizes {
        if offset + page_size > bytes.len() {
            return Err("binarycookies_parse_failed: invalid page size".to_string());
        }

        let page = &bytes[offset..offset + page_size];
        offset += page_size;

        parse_binarycookies_page(page, &mut cookies)?;
    }

    let cookie_header = cookies
        .iter()
        .map(|cookie| {
            let name = cookie
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let value = cookie
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or_default();
            format!("{name}={value}")
        })
        .filter(|item| item != "=")
        .collect::<Vec<_>>()
        .join("; ");

    let session_id = cookies.iter().find_map(|cookie| {
        let name = cookie
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if name.eq_ignore_ascii_case("sessionid") {
            cookie
                .get("value")
                .and_then(Value::as_str)
                .map(str::to_string)
        } else {
            None
        }
    });

    Ok(json!({
        "cookies": cookies,
        "cookieCount": cookies.len(),
        "cookieHeader": cookie_header,
        "sessionId": session_id,
    }))
}

fn parse_binarycookies_page(page: &[u8], cookies: &mut Vec<Value>) -> Result<(), String> {
    if page.len() < 12 {
        return Ok(());
    }

    let cookie_count = usize::try_from(read_u32_le(page, 4)?)
        .map_err(|_| "binarycookies_parse_failed: invalid cookie count".to_string())?;
    let offsets_start = 8usize;
    let offsets_end = offsets_start + cookie_count * 4;
    if offsets_end + 4 > page.len() {
        return Err("binarycookies_parse_failed: invalid cookie offsets".to_string());
    }

    let mut cookie_offsets = Vec::with_capacity(cookie_count);
    for index in 0..cookie_count {
        let cookie_offset = usize::try_from(read_u32_le(page, offsets_start + index * 4)?)
            .map_err(|_| "binarycookies_parse_failed: invalid cookie offset".to_string())?;
        cookie_offsets.push(cookie_offset);
    }

    for cookie_offset in cookie_offsets {
        if cookie_offset + 4 > page.len() {
            continue;
        }

        let cookie_size = usize::try_from(read_u32_le(page, cookie_offset)?)
            .map_err(|_| "binarycookies_parse_failed: invalid cookie size".to_string())?;
        let payload_start = cookie_offset + 4;
        let payload_end = payload_start + cookie_size;
        if payload_end > page.len() || cookie_size < 48 {
            continue;
        }

        let cookie = &page[payload_start..payload_end];
        let flags = read_u32_le(cookie, 4)?;
        let url_offset = usize::try_from(read_u32_le(cookie, 12)?)
            .map_err(|_| "binarycookies_parse_failed: invalid url offset".to_string())?;
        let name_offset = usize::try_from(read_u32_le(cookie, 16)?)
            .map_err(|_| "binarycookies_parse_failed: invalid name offset".to_string())?;
        let path_offset = usize::try_from(read_u32_le(cookie, 20)?)
            .map_err(|_| "binarycookies_parse_failed: invalid path offset".to_string())?;
        let value_offset = usize::try_from(read_u32_le(cookie, 24)?)
            .map_err(|_| "binarycookies_parse_failed: invalid value offset".to_string())?;
        let expires = read_f64_le(cookie, 36)?;
        let created = read_f64_le(cookie, 44)?;

        let domain = read_cookie_string(cookie, url_offset.saturating_sub(4));
        let name = read_cookie_string(cookie, name_offset.saturating_sub(4));
        let path = read_cookie_string(cookie, path_offset.saturating_sub(4));
        let value = read_cookie_string(cookie, value_offset.saturating_sub(4));

        cookies.push(json!({
            "name": name,
            "domain": domain,
            "path": path,
            "value": value,
            "flags": flags,
            "flagsLabel": binarycookies_flags_label(flags),
            "expires": apple_epoch_to_unix(expires),
            "expiresLabel": apple_epoch_to_label(expires),
            "created": apple_epoch_to_unix(created),
            "createdLabel": apple_epoch_to_label(created),
        }));
    }

    Ok(())
}

fn sqlite_preview(connection: &Connection) -> Result<Value, String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
             ORDER BY name LIMIT 20",
        )
        .map_err(|error| format!("sqlite_query_failed: {error}"))?;

    let table_names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| format!("sqlite_query_failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("sqlite_query_failed: {error}"))?;

    let tables = table_names
        .into_iter()
        .map(|table_name| sqlite_table_preview(connection, &table_name))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!({ "tables": tables }))
}

fn sqlite_table_preview(connection: &Connection, table_name: &str) -> Result<Value, String> {
    let escaped_name = table_name.replace('"', "\"\"");
    let pragma_sql = format!("PRAGMA table_info(\"{escaped_name}\")");

    let mut pragma_statement = connection
        .prepare(&pragma_sql)
        .map_err(|error| format!("sqlite_query_failed: {error}"))?;

    let columns = pragma_statement
        .query_map([], |row| {
            let name = row.get::<_, String>(1)?;
            let data_type = row.get::<_, String>(2).unwrap_or_default();
            Ok(json!({
                "name": name,
                "dataType": data_type,
            }))
        })
        .map_err(|error| format!("sqlite_query_failed: {error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("sqlite_query_failed: {error}"))?;

    let select_sql = format!("SELECT * FROM \"{escaped_name}\" LIMIT 20");
    let mut select_statement = connection
        .prepare(&select_sql)
        .map_err(|error| format!("sqlite_query_failed: {error}"))?;

    let column_names = select_statement
        .column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let mut rows = select_statement
        .query([])
        .map_err(|error| format!("sqlite_query_failed: {error}"))?;
    let mut preview_rows = Vec::new();

    while let Some(row) = rows
        .next()
        .map_err(|error| format!("sqlite_query_failed: {error}"))?
    {
        let mut object = Map::new();

        for (index, name) in column_names.iter().enumerate() {
            let value_ref = row
                .get_ref(index)
                .map_err(|error| format!("sqlite_query_failed: {error}"))?;
            object.insert(name.clone(), sqlite_value_to_json(value_ref));
        }

        preview_rows.push(Value::Object(object));
    }

    Ok(json!({
        "name": table_name,
        "columns": columns,
        "rows": preview_rows,
    }))
}

fn sqlite_value_to_json(value_ref: ValueRef<'_>) -> Value {
    match value_ref {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => json!(format!("<blob:{} bytes>", value.len())),
    }
}

fn collect_candidate_entries(zip_path: &str) -> Result<Vec<IndexedEntry>, String> {
    if is_backup_directory_source(zip_path)? {
        let mut entries = collect_backup_candidate_entries_from_directory(zip_path)?;
        entries.sort_by(|left, right| left.display_path.cmp(&right.display_path));
        entries.dedup_by(|left, right| left.display_path == right.display_path);
        return Ok(entries);
    }

    let mut archive = open_zip(zip_path)?;
    let mut entries = collect_direct_candidate_entries(&mut archive)?;
    entries.extend(collect_backup_candidate_entries(&mut archive)?);
    entries.sort_by(|left, right| left.display_path.cmp(&right.display_path));
    entries.dedup_by(|left, right| left.display_path == right.display_path);
    Ok(entries)
}

fn collect_direct_candidate_entries(
    archive: &mut ZipArchive<File>,
) -> Result<Vec<IndexedEntry>, String> {
    let mut entries = Vec::new();

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("zip_entry_read_failed: {error}"))?;

        if file.is_dir() {
            continue;
        }

        let inner_path = normalize_path(file.name());
        if !may_contain_tracked_app_path(&inner_path) {
            continue;
        }

        let Some((_, app_id, sandbox_path)) = split_entry_path(&inner_path) else {
            continue;
        };

        if !should_keep_app(&app_id) {
            continue;
        }

        let file_type = detect_file_type(&sandbox_path);
        let parameter_scope = classify_parameter_scope(&sandbox_path);
        if !should_analyze_app_file(&app_id, &sandbox_path, parameter_scope, file_type) {
            continue;
        }

        entries.push(IndexedEntry {
            display_path: inner_path,
            app_id,
            file_type,
            parameter_scope,
            size: file.size(),
        });
    }

    Ok(entries)
}

fn collect_backup_candidate_entries(
    archive: &mut ZipArchive<File>,
) -> Result<Vec<IndexedEntry>, String> {
    let Some(context) = load_backup_manifest_context(archive)? else {
        return Ok(Vec::new());
    };

    let mut statement = context
        .connection
        .prepare(
            "SELECT domain, relativePath, fileID \
             FROM Files \
             WHERE flags = 1 \
               AND domain IN ('AppDomain-com.ss.iphone.ugc.Aweme', 'AppDomain-com.ss.iphone.article.news')",
        )
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;

    let mut entries = Vec::new();

    for row in rows {
        let (domain, relative_path, file_id) =
            row.map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        let Some(app_id) = backup_domain_app_id(&domain) else {
            continue;
        };

        let sandbox_path = normalize_backup_relative_path(&relative_path);
        let file_type = detect_file_type(&sandbox_path);
        let parameter_scope = classify_parameter_scope(&sandbox_path);
        if !should_analyze_app_file(app_id, &sandbox_path, parameter_scope, file_type) {
            continue;
        }

        let Some(actual_path) = build_backup_actual_entry_path(&context.base_dir, &file_id) else {
            continue;
        };
        let size = match archive.by_name(&actual_path) {
            Ok(file) => file.size(),
            Err(ZipError::FileNotFound) => continue,
            Err(error) => {
                return Err(format!("backup_zip_entry_lookup_failed: {error}"));
            }
        };

        entries.push(IndexedEntry {
            display_path: build_backup_virtual_path(app_id, &sandbox_path),
            app_id: app_id.to_string(),
            file_type,
            parameter_scope,
            size,
        });
    }

    Ok(entries)
}

fn collect_backup_candidate_entries_from_directory(
    source_path: &str,
) -> Result<Vec<IndexedEntry>, String> {
    let Some(context) = load_backup_manifest_context_from_directory(source_path)? else {
        return Ok(Vec::new());
    };
    collect_backup_candidate_entries_from_context(&context, |file_id| {
        build_backup_actual_fs_path(&context.base_dir, file_id)
            .and_then(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len())
    })
}

fn collect_backup_candidate_entries_from_context<F>(
    context: &BackupManifestContext,
    mut resolve_size: F,
) -> Result<Vec<IndexedEntry>, String>
where
    F: FnMut(&str) -> Option<u64>,
{
    let mut statement = context
        .connection
        .prepare(
            "SELECT domain, relativePath, fileID \
             FROM Files \
             WHERE flags = 1 \
               AND domain IN ('AppDomain-com.ss.iphone.ugc.Aweme', 'AppDomain-com.ss.iphone.article.news')",
        )
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;

    let mut entries = Vec::new();

    for row in rows {
        let (domain, relative_path, file_id) =
            row.map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        let Some(app_id) = backup_domain_app_id(&domain) else {
            continue;
        };

        let sandbox_path = normalize_backup_relative_path(&relative_path);
        let file_type = detect_file_type(&sandbox_path);
        let parameter_scope = classify_parameter_scope(&sandbox_path);
        if !should_analyze_app_file(app_id, &sandbox_path, parameter_scope, file_type) {
            continue;
        }

        let Some(size) = resolve_size(&file_id) else {
            continue;
        };

        entries.push(IndexedEntry {
            display_path: build_backup_virtual_path(app_id, &sandbox_path),
            app_id: app_id.to_string(),
            file_type,
            parameter_scope,
            size,
        });
    }

    Ok(entries)
}

fn load_backup_manifest_context(
    archive: &mut ZipArchive<File>,
) -> Result<Option<BackupManifestContext>, String> {
    let Some(manifest_path) = find_backup_manifest_path(archive)? else {
        return Ok(None);
    };

    let mut manifest_bytes = Vec::new();
    archive
        .by_name(&manifest_path)
        .map_err(|error| format!("backup_manifest_read_failed: {error}"))?
        .read_to_end(&mut manifest_bytes)
        .map_err(|error| format!("backup_manifest_read_failed: {error}"))?;

    let temp_dir = tempdir().map_err(|error| format!("backup_manifest_extract_failed: {error}"))?;
    let manifest_db_path = temp_dir.path().join("Manifest.db");
    fs::write(&manifest_db_path, &manifest_bytes)
        .map_err(|error| format!("backup_manifest_extract_failed: {error}"))?;
    let connection = Connection::open(&manifest_db_path)
        .map_err(|error| format!("backup_manifest_open_failed: {error}"))?;
    let base_dir = manifest_path
        .trim_end_matches("Manifest.db")
        .trim_end_matches('/')
        .to_string();

    Ok(Some(BackupManifestContext {
        _temp_dir: Some(temp_dir),
        connection,
        base_dir,
    }))
}

fn load_backup_manifest_context_from_directory(
    source_path: &str,
) -> Result<Option<BackupManifestContext>, String> {
    let manifest_path = Path::new(source_path).join("Manifest.db");
    if !manifest_path.is_file() {
        return Ok(None);
    }

    let connection = Connection::open(&manifest_path)
        .map_err(|error| format!("backup_manifest_open_failed: {error}"))?;
    Ok(Some(BackupManifestContext {
        _temp_dir: None,
        connection,
        base_dir: source_path.to_string(),
    }))
}

fn find_backup_manifest_path(archive: &mut ZipArchive<File>) -> Result<Option<String>, String> {
    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("zip_entry_read_failed: {error}"))?;
        if file.is_dir() {
            continue;
        }

        let inner_path = normalize_path(file.name());
        if inner_path.ends_with("/Manifest.db") || inner_path == "Manifest.db" {
            return Ok(Some(inner_path));
        }
    }

    Ok(None)
}

fn backup_domain_app_id(domain: &str) -> Option<&str> {
    domain.strip_prefix("AppDomain-")
}

fn normalize_backup_relative_path(relative_path: &str) -> String {
    normalize_path(relative_path)
        .trim_start_matches('/')
        .to_string()
}

fn build_backup_virtual_path(app_id: &str, sandbox_path: &str) -> String {
    format!(
        "{BACKUP_VIRTUAL_ROOT}/{app_id}/{}",
        sandbox_path.trim_start_matches('/')
    )
}

fn build_backup_actual_entry_path(base_dir: &str, file_id: &str) -> Option<String> {
    let prefix = file_id.get(0..2)?;
    Some(format!("{base_dir}/{prefix}/{file_id}"))
}

fn build_app_file_path_index(zip_path: &str, app_id: &str) -> Result<Vec<String>, String> {
    let mut archive = open_zip(zip_path)?;
    let mut paths = Vec::new();

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("zip_entry_read_failed: {error}"))?;
        if file.is_dir() {
            continue;
        }
        let inner_path = normalize_path(file.name());
        let Some((_, entry_app_id, _)) = split_entry_path(&inner_path) else {
            continue;
        };
        if entry_app_id == app_id {
            paths.push(inner_path);
        }
    }

    if let Some(context) = load_backup_manifest_context(&mut archive)? {
        let domain = format!("AppDomain-{app_id}");
        let mut statement = context
            .connection
            .prepare(
                "SELECT relativePath FROM Files \
                 WHERE flags = 1 AND domain = ?1",
            )
            .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        let rows = statement
            .query_map([domain.as_str()], |row| row.get::<_, String>(0))
            .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        for row in rows {
            let relative_path =
                row.map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
            paths.push(build_backup_virtual_path(app_id, &relative_path));
        }
    }

    Ok(paths)
}

fn find_app_file_path(
    zip_path: &str,
    app_id: &str,
    suffixes: &[&str],
) -> Result<Option<String>, String> {
    let suffixes_lower = suffixes
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<Vec<_>>();

    if is_backup_directory_source(zip_path)? {
        return find_app_file_path_in_backup_directory(zip_path, app_id, &suffixes_lower);
    }

    let zip_cache_key = build_zip_cache_key(zip_path)?;
    let cache_key = build_app_file_path_index_cache_key(&zip_cache_key, app_id);
    let paths = get_or_build_app_file_path_index(&cache_key, || {
        build_app_file_path_index(zip_path, app_id)
    })?;

    Ok(paths.into_iter().find(|path| {
        let path_lower = path.to_ascii_lowercase();
        suffixes_lower
            .iter()
            .any(|suffix| path_lower.ends_with(suffix))
    }))
}

fn find_app_file_path_in_backup_directory(
    source_path: &str,
    app_id: &str,
    suffixes_lower: &[String],
) -> Result<Option<String>, String> {
    let Some(context) = load_backup_manifest_context_from_directory(source_path)? else {
        return Ok(None);
    };
    let domain = format!("AppDomain-{app_id}");
    let mut statement = context
        .connection
        .prepare(
            "SELECT relativePath FROM Files \
             WHERE flags = 1 AND domain = ?1",
        )
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
    let rows = statement
        .query_map([domain.as_str()], |row| row.get::<_, String>(0))
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;

    for row in rows {
        let relative_path =
            row.map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
        let relative_lower = relative_path.to_ascii_lowercase();
        if suffixes_lower
            .iter()
            .any(|suffix| relative_lower.ends_with(suffix))
        {
            return Ok(Some(build_backup_virtual_path(app_id, &relative_path)));
        }
    }

    Ok(None)
}

fn open_zip(zip_path: &str) -> Result<ZipArchive<File>, String> {
    let file = File::open(zip_path).map_err(|error| format!("zip_open_failed: {error}"))?;
    ZipArchive::new(file).map_err(|error| format!("zip_open_failed: {error}"))
}

fn read_zip_entry_bytes(zip_path: &str, inner_path: &str) -> Result<Vec<u8>, String> {
    if is_backup_directory_source(zip_path)? {
        return read_backup_virtual_entry_bytes_from_directory(zip_path, inner_path)?
            .ok_or_else(|| format_zip_lookup_error(inner_path, ZipError::FileNotFound));
    }

    let mut archive = open_zip(zip_path)?;
    read_entry_bytes_from_archive(&mut archive, inner_path)?
        .ok_or_else(|| format_zip_lookup_error(inner_path, ZipError::FileNotFound))
}

fn read_entry_bytes_from_archive(
    archive: &mut ZipArchive<File>,
    inner_path: &str,
) -> Result<Option<Vec<u8>>, String> {
    match archive.by_name(inner_path) {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|error| format!("zip_entry_read_failed: {error}"))?;
            return Ok(Some(bytes));
        }
        Err(ZipError::FileNotFound) => {}
        Err(error) => return Err(format_zip_lookup_error(inner_path, error)),
    }

    read_backup_virtual_entry_bytes_from_archive(archive, inner_path)
}

fn read_backup_virtual_entry_bytes_from_archive(
    archive: &mut ZipArchive<File>,
    inner_path: &str,
) -> Result<Option<Vec<u8>>, String> {
    let Some((batch_root, app_id, sandbox_path)) = split_entry_path(inner_path) else {
        return Ok(None);
    };
    if batch_root.as_deref() != Some(BACKUP_VIRTUAL_ROOT) {
        return Ok(None);
    }

    let Some(context) = load_backup_manifest_context(archive)? else {
        return Ok(None);
    };
    let domain = format!("AppDomain-{app_id}");
    let mut statement = context
        .connection
        .prepare(
            "SELECT fileID FROM Files \
             WHERE flags = 1 AND domain = ?1 AND relativePath = ?2 \
             LIMIT 1",
        )
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
    let file_id = match statement.query_row((domain.as_str(), sandbox_path.as_str()), |row| {
        row.get::<_, String>(0)
    }) {
        Ok(file_id) => file_id,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(format!("backup_manifest_query_failed: {error}")),
    };
    let Some(actual_path) = build_backup_actual_entry_path(&context.base_dir, &file_id) else {
        return Ok(None);
    };

    let mut file = archive
        .by_name(&actual_path)
        .map_err(|error| format_zip_lookup_error(inner_path, error))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("zip_entry_read_failed: {error}"))?;
    Ok(Some(bytes))
}

fn read_backup_virtual_entry_bytes_from_directory(
    source_path: &str,
    inner_path: &str,
) -> Result<Option<Vec<u8>>, String> {
    let Some((batch_root, app_id, sandbox_path)) = split_entry_path(inner_path) else {
        return Ok(None);
    };
    if batch_root.as_deref() != Some(BACKUP_VIRTUAL_ROOT) {
        return Ok(None);
    }

    let Some(context) = load_backup_manifest_context_from_directory(source_path)? else {
        return Ok(None);
    };
    let domain = format!("AppDomain-{app_id}");
    let mut statement = context
        .connection
        .prepare(
            "SELECT fileID FROM Files \
             WHERE flags = 1 AND domain = ?1 AND relativePath = ?2 \
             LIMIT 1",
        )
        .map_err(|error| format!("backup_manifest_query_failed: {error}"))?;
    let file_id = match statement.query_row((domain.as_str(), sandbox_path.as_str()), |row| {
        row.get::<_, String>(0)
    }) {
        Ok(file_id) => file_id,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(format!("backup_manifest_query_failed: {error}")),
    };
    let Some(actual_path) = build_backup_actual_fs_path(&context.base_dir, &file_id) else {
        return Ok(None);
    };
    if !actual_path.is_file() {
        return Ok(None);
    }

    fs::read(actual_path)
        .map(Some)
        .map_err(|error| format!("zip_entry_read_failed: {error}"))
}

fn extract_entry_to_path(
    archive: &mut ZipArchive<File>,
    inner_path: &str,
    output_path: &Path,
) -> Result<bool, String> {
    match archive.by_name(inner_path) {
        Ok(mut entry) => {
            let mut output = File::create(output_path)
                .map_err(|error| format!("sqlite_extract_failed: {error}"))?;
            std::io::copy(&mut entry, &mut output)
                .map_err(|error| format!("sqlite_extract_failed: {error}"))?;
            Ok(true)
        }
        Err(ZipError::FileNotFound) => Ok(false),
        Err(error) => Err(format!("sqlite_extract_failed: {error}")),
    }
}

fn extract_entry_to_path_from_source(
    source_path: &str,
    inner_path: &str,
    output_path: &Path,
) -> Result<bool, String> {
    if is_backup_directory_source(source_path)? {
        let Some(bytes) = read_backup_virtual_entry_bytes_from_directory(source_path, inner_path)?
        else {
            return Ok(false);
        };
        fs::write(output_path, bytes).map_err(|error| format!("sqlite_extract_failed: {error}"))?;
        return Ok(true);
    }

    let mut archive = open_zip(source_path)?;
    extract_entry_to_path(&mut archive, inner_path, output_path)
}

fn split_entry_path(path: &str) -> Option<(Option<String>, String, String)> {
    let normalized = normalize_path(path);
    let parts = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match parts.len() {
        0 | 1 => None,
        2 => Some((None, parts[0].to_string(), parts[1].to_string())),
        _ => Some((
            Some(parts[0].to_string()),
            parts[1].to_string(),
            parts[2..].join("/"),
        )),
    }
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn detect_file_type(sandbox_path: &str) -> &'static str {
    let lower = sandbox_path.to_ascii_lowercase();

    if lower.contains("/mmkv/") {
        "mmkv"
    } else if lower.ends_with(".plist") {
        "plist"
    } else if lower.ends_with(".json") {
        "json"
    } else if lower.ends_with(".db") || lower.ends_with(".sqlite") || lower.ends_with(".sqlite3") {
        "sqlite"
    } else if lower.ends_with(".binarycookies") {
        "binarycookies"
    } else if lower.ends_with(".archiver") {
        "archiver"
    } else {
        "unknown"
    }
}

fn should_keep_app(app_id: &str) -> bool {
    let lower = app_id.to_ascii_lowercase();
    is_tracked_app_id(&lower)
}

fn is_keychain_app(app_id: &str) -> bool {
    app_id.to_ascii_lowercase().starts_with("keychain")
}

fn is_tracked_app_id(lower_app_id: &str) -> bool {
    matches!(
        lower_app_id,
        "com.ss.iphone.ugc.aweme" | "com.ss.iphone.article.news"
    )
}

fn may_contain_tracked_app_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("com.ss.iphone.ugc.aweme/") || lower.contains("com.ss.iphone.article.news/")
}

fn should_analyze_app_file(
    app_id: &str,
    sandbox_path: &str,
    parameter_scope: &str,
    file_type: &str,
) -> bool {
    let lower_app_id = app_id.to_ascii_lowercase();
    let lower_path = sandbox_path.to_ascii_lowercase();

    if lower_app_id == "com.ss.iphone.ugc.aweme" && lower_path.ends_with("library/logindata.dat") {
        return true;
    }

    if !should_analyze_file(parameter_scope, file_type) {
        return false;
    }

    match lower_app_id.as_str() {
        "com.ss.iphone.ugc.aweme" => {
            lower_path.ends_with("library/preferences/com.ss.iphone.ugc.aweme.plist")
                || lower_path.ends_with("library/cookies/cookies.binarycookies")
                || lower_path.ends_with("documents/ttaccountsdkuserinfo.archiver")
                || lower_path.ends_with("library/logindata.dat")
        }
        "com.ss.iphone.article.news" => {
            lower_path.ends_with("library/preferences/com.ss.iphone.article.news.plist")
                || lower_path.contains("library/cookies/")
        }
        _ => false,
    }
}

fn build_app_presentation(app_id: &str) -> AppPresentation {
    let lower = app_id.to_ascii_lowercase();

    match lower.as_str() {
        "keychains" | "keychain" => AppPresentation {
            display_name: "Keychain".to_string(),
            subtitle: "iOS Keychain 数据".to_string(),
            app_kind: "keychain".to_string(),
            logo_text: "KC".to_string(),
            logo_color: "#475569".to_string(),
        },
        "com.ss.iphone.ugc.aweme" => AppPresentation {
            display_name: "抖音".to_string(),
            subtitle: app_id.to_string(),
            app_kind: "bundle".to_string(),
            logo_text: "抖".to_string(),
            logo_color: "#111827".to_string(),
        },
        "com.ss.iphone.article.news" => AppPresentation {
            display_name: "今日头条".to_string(),
            subtitle: app_id.to_string(),
            app_kind: "bundle".to_string(),
            logo_text: "头".to_string(),
            logo_color: "#ef4444".to_string(),
        },
        "com.apple.pasteboard" => AppPresentation {
            display_name: "Apple Pasteboard".to_string(),
            subtitle: app_id.to_string(),
            app_kind: "system".to_string(),
            logo_text: "PB".to_string(),
            logo_color: "#6b7280".to_string(),
        },
        _ => {
            let display_name = guess_app_name_from_bundle_id(app_id);
            let logo_text = build_logo_text(&display_name, app_id);
            let logo_color = build_logo_color(app_id);

            AppPresentation {
                display_name,
                subtitle: app_id.to_string(),
                app_kind: "bundle".to_string(),
                logo_text,
                logo_color,
            }
        }
    }
}

fn guess_app_name_from_bundle_id(app_id: &str) -> String {
    let last = app_id
        .split('.')
        .rfind(|segment| !segment.is_empty())
        .unwrap_or(app_id);
    let words = split_identifier_words(last);

    if words.is_empty() {
        app_id.to_string()
    } else {
        words
            .into_iter()
            .map(|word| capitalize_ascii(&word))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn split_identifier_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let chars = value.chars().collect::<Vec<_>>();

    for (index, ch) in chars.iter().enumerate() {
        if *ch == '_' || *ch == '-' {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
            continue;
        }

        let next_is_lower = chars
            .get(index + 1)
            .map(|next| next.is_ascii_lowercase())
            .unwrap_or(false);
        let should_split = !current.is_empty()
            && ch.is_ascii_uppercase()
            && (current.chars().last().unwrap_or('a').is_ascii_lowercase() || next_is_lower);

        if should_split {
            words.push(current.clone());
            current.clear();
        }

        current.push(*ch);
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn capitalize_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };

    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.as_str().to_ascii_lowercase()
    )
}

fn build_logo_text(display_name: &str, app_id: &str) -> String {
    let non_ascii = display_name
        .chars()
        .find(|ch| !ch.is_ascii_whitespace() && !ch.is_ascii_punctuation() && !ch.is_ascii());
    if let Some(ch) = non_ascii {
        return ch.to_string();
    }

    let initials = display_name
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>();
    if !initials.is_empty() {
        return initials.to_ascii_uppercase();
    }

    app_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(2)
        .collect::<String>()
        .to_ascii_uppercase()
}

fn build_logo_color(app_id: &str) -> String {
    const COLORS: [&str; 8] = [
        "#2563eb", "#7c3aed", "#db2777", "#ea580c", "#0891b2", "#16a34a", "#ca8a04", "#334155",
    ];

    let hash = app_id
        .bytes()
        .fold(0usize, |acc, byte| acc.wrapping_add(byte as usize));
    COLORS[hash % COLORS.len()].to_string()
}

fn classify_parameter_scope(sandbox_path: &str) -> &'static str {
    let lower = sandbox_path.to_ascii_lowercase();
    if lower.starts_with("keychain") {
        "keychain"
    } else if lower.starts_with("library/preferences/") {
        "preferences"
    } else if lower.starts_with("library/cookies/") {
        "cookies"
    } else if lower.starts_with("library/webkit/") {
        "webkit"
    } else {
        "other"
    }
}

fn parameter_scope_priority(scope: &str) -> u8 {
    match scope {
        "preferences" => 0,
        "cookies" => 1,
        "webkit" => 2,
        "keychain" => 3,
        _ => 4,
    }
}

fn should_analyze_file(parameter_scope: &str, file_type: &str) -> bool {
    parameter_scope != "other" && is_candidate_type(file_type)
}

fn resolve_scan_input(input_path: &str) -> Result<ScanInput, String> {
    let manual_zip_paths = extract_manual_zip_paths(input_path);
    if manual_zip_paths.len() > 1 {
        let mut zip_paths = Vec::with_capacity(manual_zip_paths.len());
        for zip_path in manual_zip_paths {
            let path = Path::new(&zip_path);
            let metadata =
                fs::metadata(path).map_err(|error| format!("path_stat_failed: {error}"))?;
            if !metadata.is_file() || !is_zip_file(path) {
                return Err("scan_path_failed: 拖入的项目里包含非 zip 文件".to_string());
            }
            zip_paths.push(path.to_string_lossy().to_string());
        }

        return Ok(ScanInput {
            source_mode: "files".to_string(),
            zip_paths,
        });
    }

    let path = Path::new(input_path);
    let metadata = fs::metadata(path).map_err(|error| format!("path_stat_failed: {error}"))?;

    if metadata.is_file() {
        if !is_zip_file(path) {
            return Err(
                "scan_path_failed: 请选择 zip 文件、iTunes 备份目录或包含它们的目录".to_string(),
            );
        }

        return Ok(ScanInput {
            source_mode: "zip".to_string(),
            zip_paths: vec![path.to_string_lossy().to_string()],
        });
    }

    if !metadata.is_dir() {
        return Err("scan_path_failed: 当前路径既不是文件也不是目录".to_string());
    }

    let mut zip_paths = Vec::new();
    collect_source_paths(path, &mut zip_paths)?;
    zip_paths.sort();

    if zip_paths.is_empty() {
        return Err("scan_path_failed: 目录下未找到 zip 文件或 iTunes 备份目录".to_string());
    }

    Ok(ScanInput {
        source_mode: "directory".to_string(),
        zip_paths,
    })
}

fn extract_manual_zip_paths(input: &str) -> Vec<String> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn build_chunk_ranges(total_items: usize, target_chunks: usize) -> Vec<(usize, usize)> {
    if total_items == 0 || target_chunks == 0 {
        return Vec::new();
    }

    let chunk_size = total_items.div_ceil(target_chunks);
    (0..total_items)
        .step_by(chunk_size)
        .map(|start| (start, total_items.min(start + chunk_size)))
        .collect()
}

fn collect_source_paths(dir: &Path, zip_paths: &mut Vec<String>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("scan_path_failed: {error}"))?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("scan_path_failed: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("scan_path_failed: {error}"))?;

        if file_type.is_file() && is_zip_file(&path) {
            zip_paths.push(path.to_string_lossy().to_string());
            continue;
        }

        if file_type.is_dir() {
            if is_backup_root_directory_path(&path) {
                zip_paths.push(path.to_string_lossy().to_string());
                continue;
            }

            collect_source_paths(&path, zip_paths)?;
        }
    }

    Ok(())
}

fn is_zip_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn is_backup_root_directory_path(path: &Path) -> bool {
    path.join("Manifest.db").is_file()
}

fn is_backup_directory_source(source_path: &str) -> Result<bool, String> {
    let metadata =
        fs::metadata(source_path).map_err(|error| format!("path_stat_failed: {error}"))?;
    Ok(metadata.is_dir() && is_backup_root_directory_path(Path::new(source_path)))
}

fn build_backup_actual_fs_path(base_dir: &str, file_id: &str) -> Option<PathBuf> {
    let prefix = file_id.get(0..2)?;
    Some(Path::new(base_dir).join(prefix).join(file_id))
}

fn build_scan_cache_key(input_path: &str, zip_paths: &[String]) -> Result<String, String> {
    let mut keys = zip_paths
        .iter()
        .map(|zip_path| build_zip_cache_key(zip_path))
        .collect::<Result<Vec<_>, _>>()?;
    keys.sort();
    Ok(format!(
        "scan::focused-v2::{input_path}::{}",
        keys.join("||")
    ))
}

fn build_zip_cache_key(zip_path: &str) -> Result<String, String> {
    let metadata = fs::metadata(zip_path).map_err(|error| format!("zip_stat_failed: {error}"))?;
    let modified = metadata
        .modified()
        .map_err(|error| format!("zip_stat_failed: {error}"))?;
    let modified_nanos = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("zip_stat_failed: {error}"))?
        .as_nanos();
    Ok(format!("{zip_path}::{modified_nanos}"))
}

fn build_files_cache_key(zip_cache_key: &str, app_id: &str) -> String {
    format!("{zip_cache_key}::files::focused-v2::{app_id}")
}

fn build_parse_cache_key(zip_cache_key: &str, inner_path: &str) -> String {
    format!("{zip_cache_key}::parse::{inner_path}")
}

fn build_app_file_path_index_cache_key(zip_cache_key: &str, app_id: &str) -> String {
    format!("{zip_cache_key}::app-file-paths::v1::{app_id}")
}

fn with_scan_cache_hit(mut summary: ZipScanSummary, cache_hit: bool) -> ZipScanSummary {
    summary.cache_hit = cache_hit;
    summary
}

fn with_parse_cache_hit(mut result: ParseResult, cache_hit: bool) -> ParseResult {
    if let Some(meta) = result.meta.as_object_mut() {
        meta.insert("cacheHit".to_string(), json!(cache_hit));
    } else {
        result.meta = json!({ "cacheHit": cache_hit });
    }
    result
}

fn cache_get_scan(cache_key: &str) -> Result<Option<ZipScanSummary>, String> {
    let cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    Ok(cache.scan_cache.get(cache_key).cloned())
}

fn cache_put_scan(cache_key: String, summary: ZipScanSummary) -> Result<(), String> {
    let mut cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    cache.scan_cache.insert(cache_key, summary);
    Ok(())
}

fn cache_get_files(cache_key: &str) -> Result<Option<Vec<CandidateFile>>, String> {
    let cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    Ok(cache.files_cache.get(cache_key).cloned())
}

fn cache_put_files(cache_key: String, files: Vec<CandidateFile>) -> Result<(), String> {
    let mut cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    cache.files_cache.insert(cache_key, files);
    Ok(())
}

fn cache_get_parse(cache_key: &str) -> Result<Option<ParseResult>, String> {
    let cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    Ok(cache.parse_cache.get(cache_key).cloned())
}

fn cache_put_parse(cache_key: String, result: ParseResult) -> Result<(), String> {
    let mut cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    cache.parse_cache.insert(cache_key, result);
    Ok(())
}

fn cache_get_app_file_path_index(cache_key: &str) -> Result<Option<Vec<String>>, String> {
    let cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    Ok(cache.app_file_path_indexes.get(cache_key).cloned())
}

fn cache_put_app_file_path_index(cache_key: String, paths: Vec<String>) -> Result<(), String> {
    let mut cache = CACHE_STATE
        .lock()
        .map_err(|_| "cache_lock_failed".to_string())?;
    cache.app_file_path_indexes.insert(cache_key, paths);
    Ok(())
}

fn get_or_build_app_file_path_index<F>(cache_key: &str, build: F) -> Result<Vec<String>, String>
where
    F: FnOnce() -> Result<Vec<String>, String>,
{
    if let Ok(Some(cached)) = cache_get_app_file_path_index(cache_key) {
        return Ok(cached);
    }
    let paths = build()?;
    let _ = cache_put_app_file_path_index(cache_key.to_string(), paths.clone());
    Ok(paths)
}

fn resolve_export_path(
    zip_path: &str,
    output_path: Option<String>,
    suggested_name: &str,
) -> Result<PathBuf, String> {
    if let Some(output_path) = output_path {
        let path = PathBuf::from(output_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("export_create_dir_failed: {error}"))?;
        }
        return Ok(path);
    }

    let zip_parent = Path::new(zip_path)
        .parent()
        .ok_or_else(|| "export_path_failed: zip parent not found".to_string())?;
    let export_dir = zip_parent.join("exports");
    fs::create_dir_all(&export_dir)
        .map_err(|error| format!("export_create_dir_failed: {error}"))?;
    Ok(export_dir.join(suggested_name))
}

fn write_json_export(
    output_path: PathBuf,
    payload: &Value,
    item_count: usize,
) -> Result<ExportResult, String> {
    let bytes = serde_json::to_vec_pretty(payload)
        .map_err(|error| format!("export_serialize_failed: {error}"))?;
    fs::write(&output_path, &bytes).map_err(|error| format!("export_write_failed: {error}"))?;
    Ok(ExportResult {
        output_path: output_path.to_string_lossy().to_string(),
        item_count,
        bytes_written: bytes.len(),
    })
}

fn sanitize_file_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>();
    sanitized.trim_matches('_').to_string()
}

fn export_source_prefix(zip_path: &str) -> String {
    Path::new(zip_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(sanitize_file_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "zip".to_string())
}

fn now_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

fn is_parse_supported(file_type: &str) -> bool {
    matches!(file_type, "plist" | "json" | "sqlite" | "binarycookies")
}

fn is_candidate_type(file_type: &str) -> bool {
    !matches!(file_type, "unknown")
}

fn preview_string(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn read_u32_be(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "binarycookies_parse_failed: unexpected eof".to_string())?;
    Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "binarycookies_parse_failed: unexpected eof".to_string())?;
    Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_f64_le(bytes: &[u8], offset: usize) -> Result<f64, String> {
    let slice = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| "binarycookies_parse_failed: unexpected eof".to_string())?;
    Ok(f64::from_bits(u64::from_le_bytes([
        slice[0], slice[1], slice[2], slice[3], slice[4], slice[5], slice[6], slice[7],
    ])))
}

fn read_cookie_string(bytes: &[u8], offset: usize) -> String {
    let Some(slice) = bytes.get(offset..) else {
        return String::new();
    };
    let end = slice
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..end]).trim().to_string()
}

fn douyin_json_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn douyin_json_string(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(douyin_normalize_json_value)
}

fn douyin_normalize_json_value(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).ok(),
    }
}

fn extract_douyin_session_id(cookie_header: &str) -> Option<String> {
    for key in &["sessionid", "sessionid_ss", "sid_tt"] {
        if let Some(val) = extract_cookie_value(cookie_header, key) {
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

fn extract_cookie_value(cookie_header: &str, key: &str) -> Option<String> {
    let target = format!("{key}=");
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix(&target) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn douyin_cookie_value(cookie_header: &str, key: &str) -> Option<String> {
    extract_cookie_value(cookie_header, key)
}

fn toutiao_token_value(source: &Value) -> String {
    first_non_empty_strings(&[
        source
            .get("kTTAccountTokenGuardXTTToken")
            .and_then(douyin_normalize_json_value),
        source
            .get("bdaccount_session_x_tt_token")
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default()
}

fn toutiao_device_id(source: &Value) -> String {
    first_non_empty_strings(&[
        douyin_json_value(source, &["FlowSaveDeviceId", "deviceId"])
            .and_then(douyin_normalize_json_value),
        source
            .get("kOldDeviceIDStorageKey")
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default()
}

fn toutiao_cookie_value(parsed_cookies: &Value, key: &str) -> Option<String> {
    let best_cookie = parsed_cookies
        .get("cookies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|cookie| {
            let name = cookie.get("name").and_then(Value::as_str)?;
            let value = cookie.get("value").and_then(Value::as_str)?.trim();
            if !name.eq_ignore_ascii_case(key) || value.is_empty() {
                return None;
            }
            let domain = cookie
                .get("domain")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let domain_priority = if domain.contains("toutiaoapi.com")
                || domain.contains("toutiao.com")
                || domain.contains("snssdk.com")
            {
                1_u8
            } else {
                0_u8
            };
            let created = cookie
                .get("created")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            Some((domain_priority, created, value.to_string()))
        })
        .max_by(|left, right| {
            left.0.cmp(&right.0).then_with(|| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        })
        .map(|(_, _, value)| value);

    best_cookie.or_else(|| {
        parsed_cookies
            .get("cookieHeader")
            .and_then(Value::as_str)
            .and_then(|header| extract_cookie_value(header, key))
    })
}

fn douyin_ticket_guard_info(source: &Value) -> Value {
    let Some(sec_user_dic) = source.get("kTTAccountTicketGuardSecUserIdTsSignDic") else {
        return Value::Object(Map::new());
    };
    let Some(object) = sec_user_dic.as_object() else {
        return Value::Object(Map::new());
    };
    let raw_value = object.values().find_map(Value::as_str).unwrap_or_default();
    if raw_value.is_empty() {
        return Value::Object(Map::new());
    }
    serde_json::from_str(raw_value).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn douyin_sec_user_id(source: &Value) -> Option<String> {
    source
        .get("kTTAccountTicketGuardSecUserIdTsSignDic")
        .and_then(Value::as_object)
        .and_then(|object| object.keys().next().cloned())
        .filter(|value| !value.is_empty())
}

fn douyin_extra_info(source: &Value) -> Value {
    match source.get("extra_info") {
        Some(Value::String(text)) if !text.trim().is_empty() => {
            serde_json::from_str(text).unwrap_or_else(|_| Value::Object(Map::new()))
        }
        Some(Value::Object(_)) => source.get("extra_info").cloned().unwrap_or(Value::Null),
        _ => Value::Object(Map::new()),
    }
}

fn douyin_store_region(source: &Value, cookie_header: &str) -> Option<String> {
    if let Some(region) = douyin_cookie_value(cookie_header, "store-region") {
        return Some(region);
    }

    source
        .get("kTTInstallAppRegion")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

enum ParamCandidate {
    Path(Vec<&'static str>),
    Computed(Option<String>),
}

fn douyin_resolve_request_param(
    source: &Value,
    _cookie_header: &str,
    _ticket_guard_info: &Value,
    candidates: &[ParamCandidate],
) -> Option<String> {
    for candidate in candidates {
        let value = match candidate {
            ParamCandidate::Path(path) => {
                douyin_json_value(source, path).and_then(douyin_normalize_json_value)
            }
            ParamCandidate::Computed(value) => value.clone(),
        };

        if let Some(value) = value.filter(|value| !value.is_empty()) {
            return Some(value);
        }
    }

    None
}

fn douyin_build_common_params_v2(source: &Value, cookie_header: &str) -> Option<String> {
    let extra_info = douyin_extra_info(source);
    let sec_user_id = douyin_sec_user_id(source).unwrap_or_default();
    let install_id = douyin_cookie_value(cookie_header, "install_id").unwrap_or_default();
    let app_version = first_non_empty_strings(&[
        douyin_json_value(&extra_info, &["app_version"]).and_then(douyin_normalize_json_value),
        source
            .get("kTTInstallAppVersion")
            .and_then(douyin_normalize_json_value),
        source
            .get("bdaccount_x_tt_token_app_version")
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default();
    let source_aid = source
        .get("kBDUGPushSDKAID")
        .and_then(douyin_normalize_json_value)
        .unwrap_or_else(|| "1128".to_string());

    let specs: [(&str, Option<String>); 38] = [
        (
            "device_id",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["device_id"])
                    .and_then(douyin_normalize_json_value),
                douyin_json_value(source, &["FlowSaveDeviceId", "deviceId"])
                    .and_then(douyin_normalize_json_value),
            ]),
        ),
        (
            "os_version",
            douyin_json_value(&extra_info, &["os_version"]).and_then(douyin_normalize_json_value),
        ),
        (
            "iid",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["iid"]).and_then(douyin_normalize_json_value),
                Some(install_id.clone()).filter(|value| !value.is_empty()),
            ]),
        ),
        (
            "app_name",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["app_name"]).and_then(douyin_normalize_json_value),
                Some("aweme".to_string()),
            ]),
        ),
        (
            "ac",
            douyin_json_value(&extra_info, &["ac"]).and_then(douyin_normalize_json_value),
        ),
        (
            "appTheme",
            douyin_json_value(&extra_info, &["appTheme"]).and_then(douyin_normalize_json_value),
        ),
        (
            "js_sdk_version",
            douyin_json_value(&extra_info, &["js_sdk_version"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "version_code",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["version_code"])
                    .and_then(douyin_normalize_json_value),
                Some(app_version.clone()).filter(|value| !value.is_empty()),
            ]),
        ),
        (
            "channel",
            douyin_json_value(&extra_info, &["channel"]).and_then(douyin_normalize_json_value),
        ),
        (
            "is_vcd",
            douyin_json_value(&extra_info, &["is_vcd"]).and_then(douyin_normalize_json_value),
        ),
        (
            "tma_jssdk_version",
            douyin_json_value(&extra_info, &["tma_jssdk_version"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "os_api",
            douyin_json_value(&extra_info, &["os_api"]).and_then(douyin_normalize_json_value),
        ),
        (
            "need_personal_recommend",
            douyin_json_value(&extra_info, &["need_personal_recommend"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "device_platform",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["device_platform"])
                    .and_then(douyin_normalize_json_value),
                Some("iphone".to_string()),
            ]),
        ),
        (
            "device_type",
            douyin_json_value(&extra_info, &["device_type"]).and_then(douyin_normalize_json_value),
        ),
        (
            "is_guest_mode",
            douyin_json_value(&extra_info, &["is_guest_mode"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "build_number",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["build_number"])
                    .and_then(douyin_normalize_json_value),
                source
                    .get("gurd_kit_update_version_code")
                    .and_then(douyin_normalize_json_value),
            ]),
        ),
        (
            "minor_status",
            douyin_json_value(&extra_info, &["minor_status"]).and_then(douyin_normalize_json_value),
        ),
        (
            "aid",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["aid"]).and_then(douyin_normalize_json_value),
                Some(source_aid.clone()),
            ]),
        ),
        (
            "mcc_mnc",
            Some(
                douyin_json_value(&extra_info, &["mcc_mnc"])
                    .and_then(douyin_normalize_json_value)
                    .unwrap_or_default(),
            ),
        ),
        (
            "screen_width",
            douyin_json_value(&extra_info, &["screen_width"]).and_then(douyin_normalize_json_value),
        ),
        (
            "package",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["package"]).and_then(douyin_normalize_json_value),
                Some("com.ss.iphone.ugc.Aweme".to_string()),
            ]),
        ),
        (
            "cdid",
            douyin_json_value(&extra_info, &["cdid"]).and_then(douyin_normalize_json_value),
        ),
        ("app_version", Some(app_version.clone())),
        (
            "user_avatar_shrink",
            douyin_json_value(&extra_info, &["user_avatar_shrink"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "luckydog_base",
            douyin_json_value(&extra_info, &["luckydog_base"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "luckydog_data",
            douyin_json_value(&extra_info, &["luckydog_data"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "luckydog_token",
            douyin_json_value(&extra_info, &["luckydog_token"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "card_style",
            douyin_json_value(&extra_info, &["card_style"]).and_then(douyin_normalize_json_value),
        ),
        (
            "user_id",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["user_id"]).and_then(douyin_normalize_json_value),
                Some(sec_user_id.clone()).filter(|value| !value.is_empty()),
            ]),
        ),
        (
            "source",
            douyin_json_value(&extra_info, &["source"]).and_then(douyin_normalize_json_value),
        ),
        (
            "address_book_access",
            douyin_json_value(&extra_info, &["address_book_access"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "sec_user_id",
            first_non_empty_strings(&[
                douyin_json_value(&extra_info, &["sec_user_id"])
                    .and_then(douyin_normalize_json_value),
                Some(sec_user_id.clone()).filter(|value| !value.is_empty()),
            ]),
        ),
        (
            "hit_ab_test",
            douyin_json_value(&extra_info, &["hit_ab_test"]).and_then(douyin_normalize_json_value),
        ),
        (
            "publish_video_strategy_type",
            douyin_json_value(&extra_info, &["publish_video_strategy_type"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "user_cover_shrink",
            douyin_json_value(&extra_info, &["user_cover_shrink"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "card_partition",
            douyin_json_value(&extra_info, &["card_partition"])
                .and_then(douyin_normalize_json_value),
        ),
        (
            "btn_in_value",
            douyin_json_value(&extra_info, &["btn_in_value"]).and_then(douyin_normalize_json_value),
        ),
    ];

    let parts = specs
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{key}={}",
                urlencoding::encode(value.unwrap_or_default().as_str())
            )
        })
        .collect::<Vec<_>>();

    Some(parts.join("&"))
}

fn format_douyin_request_error_message(
    endpoint: DouyinTokenEndpoint,
    error_message: &str,
) -> String {
    let name = endpoint.name();
    if let Some(idx) = error_message.find("): ") {
        format!("{}_request_failed: {}", name, &error_message[idx + 3..])
    } else if error_message.starts_with("error sending request for url (")
        && error_message.ends_with(')')
    {
        format!("{}_request_failed: request_send_failed", name)
    } else if let Some(idx) = error_message.find(" for url (") {
        let reason = error_message[..idx].trim();
        if reason.is_empty() {
            format!("{}_request_failed: {}", name, error_message)
        } else {
            format!("{}_request_failed: {}", name, reason)
        }
    } else {
        format!("{}_request_failed: {}", name, error_message)
    }
}

fn request_douyin_token_endpoint(
    client: &Client,
    endpoint: DouyinTokenEndpoint,
    source: &Value,
    cookie_header: &str,
    x_tt_token: &str,
    odin_tt: &str,
) -> DouyinTokenEndpointResult {
    let query = douyin_build_token_check_query(source, cookie_header, endpoint);
    let url = format!("{}?{query}", endpoint.base_url());
    let response = client
        .get(&url)
        .header("Accept", "*/*")
        .header("Accept-Encoding", "identity")
        .header("Cookie", douyin_token_cookie_header(cookie_header, odin_tt))
        .header("User-Agent", douyin_aweme_user_agent(source))
        .header(
            "X-SS-DP",
            first_non_empty_strings(&[
                source
                    .get("kBDUGPushSDKAID")
                    .and_then(douyin_normalize_json_value),
                Some("1128".to_string()),
            ])
            .unwrap_or_else(|| "1128".to_string()),
        )
        .header("sdk-version", endpoint.sdk_version())
        .header("x-Tt-Token", x_tt_token)
        .send();

    let Ok(response) = response else {
        return DouyinTokenEndpointResult {
            name: endpoint.name().to_string(),
            url,
            http_status: None,
            status_code: None,
            status: "request_error".to_string(),
            message: Some(
                response
                    .err()
                    .map(|error| format_douyin_request_error_message(endpoint, &error.to_string()))
                    .unwrap_or_else(|| "request_failed".to_string()),
            ),
            uid: None,
            sec_uid: None,
            nickname: None,
            phone_number: None,
            register_time: None,
            aweme_count: None,
            following_count: None,
            liked_count: None,
            functions: Vec::new(),
        };
    };

    let http_status = response.status();
    if !http_status.is_success() {
        return DouyinTokenEndpointResult {
            name: endpoint.name().to_string(),
            url,
            http_status: Some(http_status.as_u16()),
            status_code: None,
            status: "http_error".to_string(),
            message: Some(format!("http_status_{http_status}")),
            uid: None,
            sec_uid: None,
            nickname: None,
            phone_number: None,
            register_time: None,
            aweme_count: None,
            following_count: None,
            liked_count: None,
            functions: Vec::new(),
        };
    }

    let payload = response.json::<Value>();
    let Ok(payload) = payload else {
        return DouyinTokenEndpointResult {
            name: endpoint.name().to_string(),
            url,
            http_status: Some(http_status.as_u16()),
            status_code: None,
            status: "parse_error".to_string(),
            message: Some(
                payload
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "json_decode_failed".to_string()),
            ),
            uid: None,
            sec_uid: None,
            nickname: None,
            phone_number: None,
            register_time: None,
            aweme_count: None,
            following_count: None,
            liked_count: None,
            functions: Vec::new(),
        };
    };

    let parsed = parse_douyin_token_check_payload(&payload);
    let status = match parsed.is_valid {
        Some(true) => "ok",
        Some(false) => "invalid",
        None => "parse_error",
    }
    .to_string();
    let message = parsed.message.clone().or_else(|| {
        if status == "parse_error" {
            Some("douyin_token_status_not_found".to_string())
        } else {
            None
        }
    });

    DouyinTokenEndpointResult {
        name: endpoint.name().to_string(),
        url,
        http_status: Some(http_status.as_u16()),
        status_code: parsed.status_code,
        status,
        message,
        uid: parsed.uid,
        sec_uid: parsed.sec_uid,
        nickname: parsed.nickname,
        phone_number: parsed.phone_number,
        register_time: parsed.register_time,
        aweme_count: parsed.aweme_count,
        following_count: parsed.following_count,
        liked_count: parsed.liked_count,
        functions: parsed.functions,
    }
}

fn douyin_token_value(source: &Value) -> String {
    first_non_empty_strings(&[
        source
            .get("kTTAccountTokenGuardXTTToken")
            .and_then(douyin_normalize_json_value),
        source
            .get("bdaccount_session_x_tt_token")
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default()
}

fn is_douyin_act_token(token: &str) -> bool {
    token
        .trim()
        .get(..3)
        .map(|prefix| prefix.eq_ignore_ascii_case("act"))
        .unwrap_or(false)
}

fn douyin_build_token_check_query(
    source: &Value,
    cookie_header: &str,
    endpoint: DouyinTokenEndpoint,
) -> String {
    let extra_info = douyin_extra_info(source);
    let app_version = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "app_version"),
        source
            .get("kTTInstallAppVersion")
            .and_then(douyin_normalize_json_value),
        douyin_context_value(source, &extra_info, "version_code"),
        Some("23.2.0".to_string()),
    ])
    .unwrap_or_else(|| "23.2.0".to_string());
    let build_number = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "build_number"),
        source
            .get("gurd_kit_update_version_code")
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default();
    let aid = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "aid"),
        source
            .get("kBDUGPushSDKAID")
            .and_then(douyin_normalize_json_value),
        Some("1128".to_string()),
    ])
    .unwrap_or_else(|| "1128".to_string());
    let device_id = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "device_id"),
        douyin_json_value(source, &["FlowSaveDeviceId", "deviceId"])
            .and_then(douyin_normalize_json_value),
    ]);
    let install_id = douyin_cookie_value(cookie_header, "install_id");
    let iid = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "iid"),
        install_id.clone(),
    ]);

    let specs = match endpoint {
        DouyinTokenEndpoint::SafetyPortrait => vec![
            (
                "package",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "package"),
                    Some("com.ss.iphone.ugc.Aweme".to_string()),
                ]),
            ),
            (
                "appTheme",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "appTheme"),
                    Some("light".to_string()),
                ]),
            ),
            (
                "version_code",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "version_code"),
                    Some(app_version.clone()),
                ]),
            ),
            (
                "need_personal_recommend",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "need_personal_recommend"),
                    Some("1".to_string()),
                ]),
            ),
            (
                "js_sdk_version",
                douyin_context_value(source, &extra_info, "js_sdk_version"),
            ),
            (
                "tma_jssdk_version",
                douyin_context_value(source, &extra_info, "tma_jssdk_version"),
            ),
            (
                "app_name",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "app_name"),
                    Some("aweme".to_string()),
                ]),
            ),
            ("app_version", Some(app_version.clone())),
            ("device_id", device_id.clone()),
            (
                "channel",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "channel"),
                    Some("App Store".to_string()),
                ]),
            ),
            (
                "slide_guide_has_shown",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "slide_guide_has_shown"),
                    Some("1".to_string()),
                ]),
            ),
            (
                "mcc_mnc",
                douyin_context_value(source, &extra_info, "mcc_mnc"),
            ),
            ("aid", Some(aid.clone())),
            (
                "minor_status",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "minor_status"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "screen_width",
                douyin_context_value(source, &extra_info, "screen_width"),
            ),
            ("cdid", douyin_context_value(source, &extra_info, "cdid")),
            (
                "os_api",
                douyin_context_value(source, &extra_info, "os_api"),
            ),
            (
                "ac",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "ac"),
                    Some("WIFI".to_string()),
                ]),
            ),
            (
                "os_version",
                douyin_context_value(source, &extra_info, "os_version"),
            ),
            (
                "is_guest_mode",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "is_guest_mode"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "device_platform",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "device_platform"),
                    Some("iphone".to_string()),
                ]),
            ),
            ("build_number", Some(build_number.clone())),
            ("iid", iid.clone()),
            (
                "is_vcd",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "is_vcd"),
                    Some("1".to_string()),
                ]),
            ),
            (
                "device_type",
                douyin_context_value(source, &extra_info, "device_type"),
            ),
        ],
        DouyinTokenEndpoint::ProfileSelf => vec![
            (
                "ac",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "ac"),
                    Some("5G".to_string()),
                ]),
            ),
            ("aid", Some(aid.clone())),
            (
                "appTheme",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "appTheme"),
                    Some("light".to_string()),
                ]),
            ),
            (
                "app_name",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "app_name"),
                    Some("aweme".to_string()),
                ]),
            ),
            ("app_version", Some(app_version.clone())),
            ("build_number", Some(build_number.clone())),
            ("cdid", douyin_context_value(source, &extra_info, "cdid")),
            (
                "channel",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "channel"),
                    Some("App Store".to_string()),
                ]),
            ),
            ("device_id", device_id.clone()),
            (
                "device_platform",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "device_platform"),
                    Some("iphone".to_string()),
                ]),
            ),
            (
                "device_type",
                douyin_context_value(source, &extra_info, "device_type"),
            ),
            (
                "gold_container",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "gold_container"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "idfa",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "idfa"),
                    Some("00000000-0000-0000-0000-000000000000".to_string()),
                ]),
            ),
            ("iid", iid.clone()),
            (
                "in_sp_time",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "in_sp_time"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "js_sdk_version",
                douyin_context_value(source, &extra_info, "js_sdk_version"),
            ),
            (
                "language",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "language"),
                    Some("zh-Hans".to_string()),
                ]),
            ),
            (
                "mcc_mnc",
                douyin_context_value(source, &extra_info, "mcc_mnc"),
            ),
            (
                "minor_status",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "minor_status"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "openudid",
                douyin_context_value(source, &extra_info, "openudid"),
            ),
            (
                "os_api",
                douyin_context_value(source, &extra_info, "os_api"),
            ),
            (
                "os_version",
                douyin_context_value(source, &extra_info, "os_version"),
            ),
            (
                "pass-region",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "pass-region"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "pass-route",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "pass-route"),
                    Some("0".to_string()),
                ]),
            ),
            (
                "screen_width",
                douyin_context_value(source, &extra_info, "screen_width"),
            ),
            (
                "tma_jssdk_version",
                douyin_context_value(source, &extra_info, "tma_jssdk_version"),
            ),
            (
                "version_code",
                first_non_empty_strings(&[
                    douyin_context_value(source, &extra_info, "version_code"),
                    Some(app_version),
                ]),
            ),
            ("vid", douyin_context_value(source, &extra_info, "vid")),
        ],
    };

    douyin_query_from_specs(specs)
}

fn douyin_context_value(source: &Value, extra_info: &Value, key: &str) -> Option<String> {
    extra_info
        .get(key)
        .and_then(douyin_normalize_json_value)
        .or_else(|| source.get(key).and_then(douyin_normalize_json_value))
        .filter(|value| !value.is_empty())
}

fn douyin_query_from_specs(specs: Vec<(&str, Option<String>)>) -> String {
    specs
        .into_iter()
        .map(|(key, value)| {
            format!(
                "{key}={}",
                urlencoding::encode(value.unwrap_or_default().as_str())
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

fn douyin_aweme_user_agent(source: &Value) -> String {
    let extra_info = douyin_extra_info(source);
    let app_version = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "app_version"),
        source
            .get("kTTInstallAppVersion")
            .and_then(douyin_normalize_json_value),
        Some("23.2.0".to_string()),
    ])
    .unwrap_or_else(|| "23.2.0".to_string());
    let build_number = first_non_empty_strings(&[
        douyin_context_value(source, &extra_info, "build_number"),
        source
            .get("gurd_kit_update_version_code")
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default();
    let device_type = douyin_context_value(source, &extra_info, "device_type")
        .unwrap_or_else(|| "iPhone".to_string());
    let os_version = douyin_context_value(source, &extra_info, "os_version")
        .unwrap_or_else(|| "15.6.1".to_string());

    format!("Aweme {app_version} rv:{build_number} ({device_type}; iOS {os_version}; zh_CN) Cronet")
}

fn douyin_token_cookie_header(cookie_header: &str, odin_tt: &str) -> String {
    ["odin_tt", "store-region", "store-region-src"]
        .into_iter()
        .filter_map(|key| {
            let value = if key == "odin_tt" {
                Some(odin_tt.to_string()).filter(|value| !value.is_empty())
            } else {
                douyin_cookie_value(cookie_header, key)
            }?;
            Some(format!("{key}={value}"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn read_douyin_local_phone_number(zip_path: &str) -> Result<Option<String>, String> {
    let account_phone_number = read_douyin_local_account_payload(zip_path)?
        .and_then(|payload| parse_douyin_mobile_change_payload(&payload));

    let local_plist_path = find_app_file_path(
        zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Preferences/douyin_mobile_change.plist"],
    )?;
    let Some(local_plist_path) = local_plist_path else {
        return Ok(account_phone_number);
    };

    let plist_bytes = read_zip_entry_bytes(zip_path, &local_plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|error| format!("douyin_local_phone_parse_failed: {error}"))?;
    let payload = serde_json::to_value(plist_value)
        .map_err(|error| format!("douyin_local_phone_parse_failed: {error}"))?;
    let mobile_change_phone_number = parse_douyin_mobile_change_payload(&payload);
    Ok(prefer_better_phone_number(
        account_phone_number,
        mobile_change_phone_number,
    ))
}

fn parse_douyin_multi_session_map(encoded: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let decoded = match urlencoding::decode(encoded) {
        Ok(value) => value.into_owned(),
        Err(_) => encoded.to_string(),
    };

    for entry in decoded.split('|') {
        let Some((uid, session_id)) = entry.split_once(':') else {
            continue;
        };
        let uid = uid.trim();
        let session_id = session_id.trim();
        if uid.is_empty() || session_id.is_empty() {
            continue;
        }
        result.insert(uid.to_string(), session_id.to_string());
    }

    result
}

fn extract_douyin_token_cluster_map(
    plist_bytes: &[u8],
) -> BTreeMap<String, DouyinTokenClusterEntry> {
    let mut result = BTreeMap::new();
    let text = String::from_utf8_lossy(plist_bytes);

    for (start, ch) in text.char_indices() {
        if ch != '{' {
            continue;
        }
        let candidate = &text[start..];
        let mut chars = candidate.chars();
        let is_uid_key_candidate = matches!(
            (chars.next(), chars.next(), chars.next()),
            (Some('{'), Some('"'), Some(next)) if next.is_ascii_digit()
        );
        if !is_uid_key_candidate {
            continue;
        }
        let mut stream = serde_json::Deserializer::from_str(candidate).into_iter::<Value>();
        let Some(Ok(value)) = stream.next() else {
            continue;
        };
        let Some(object) = value.as_object() else {
            continue;
        };
        let looks_like_cluster = object.iter().all(|(uid, item)| {
            uid.chars().all(|char| char.is_ascii_digit())
                && item
                    .as_object()
                    .and_then(|row| row.get("accessToken"))
                    .and_then(Value::as_str)
                    .map(|token| token.starts_with("act."))
                    .unwrap_or(false)
        });
        if !looks_like_cluster {
            continue;
        }

        for (uid, item) in object {
            let Some(row) = item.as_object() else {
                continue;
            };
            let access_token = row
                .get("accessToken")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let open_id = row
                .get("openID")
                .or_else(|| row.get("openId"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let sec_uid = row
                .get("secUID")
                .or_else(|| row.get("secUid"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let auth_time_label = row
                .get("authTime")
                .and_then(Value::as_f64)
                .map(format_douyin_auth_time_label)
                .unwrap_or_default();
            result.insert(
                uid.clone(),
                DouyinTokenClusterEntry {
                    access_token,
                    open_id,
                    sec_uid,
                    auth_time_label,
                },
            );
        }
        break;
    }

    result
}

fn read_douyin_mmkv_default_bytes(zip_path: &str) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = find_app_file_path(
        zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Documents/mmkv/mmkv.default"],
    )?
    else {
        return Ok(None);
    };

    read_zip_entry_bytes(zip_path, &path).map(Some)
}

fn read_douyin_accountsaaskit_bytes(zip_path: &str) -> Result<Option<Vec<u8>>, String> {
    let Some(path) = find_app_file_path(
        zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Documents/mmkv/com.bytedance.ies.accountsaaskit"],
    )?
    else {
        return Ok(None);
    };

    read_zip_entry_bytes(zip_path, &path).map(Some)
}

fn extract_uid_sec_uid_pairs_from_accountsaaskit(dat_bytes: &[u8]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut search_start = 0usize;
    let sec_uid_prefix = b"MS4wLjAB";

    while let Some(relative) =
        find_bytes(dat_bytes.get(search_start..).unwrap_or(&[]), sec_uid_prefix)
    {
        let sec_pos = search_start + relative;
        let Some(sec_uid) = extract_sec_uid_token(dat_bytes, sec_pos) else {
            search_start = sec_pos.saturating_add(sec_uid_prefix.len());
            continue;
        };

        let window_start = sec_pos.saturating_sub(96);
        let prefix = &dat_bytes[window_start..sec_pos];
        let mut i = 0usize;
        let mut uid_candidate = None;
        while i < prefix.len() {
            if prefix[i].is_ascii_digit() {
                let start = i;
                while i < prefix.len() && prefix[i].is_ascii_digit() {
                    i += 1;
                }
                let end = i;
                let len = end - start;
                let gap = prefix.len().saturating_sub(end);
                if (15..=20).contains(&len)
                    && gap <= 12
                    && std::str::from_utf8(&prefix[start..end])
                        .ok()
                        .map(is_plausible_login_data_user_id)
                        .unwrap_or(false)
                {
                    uid_candidate = std::str::from_utf8(&prefix[start..end])
                        .ok()
                        .map(|value| value.to_string());
                }
            } else {
                i += 1;
            }
        }

        if let Some(uid) = uid_candidate {
            result.entry(uid).or_insert(sec_uid);
        }

        search_start = sec_pos.saturating_add(sec_uid_prefix.len());
    }

    result
}

fn extract_sec_uid_token(dat_bytes: &[u8], start: usize) -> Option<String> {
    let slice = dat_bytes.get(start..)?;
    if !slice.starts_with(b"MS4wLjAB") {
        return None;
    }

    let len = slice
        .iter()
        .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        .count();
    if len < 20 {
        return None;
    }

    std::str::from_utf8(&slice[..len])
        .ok()
        .map(|value| value.to_string())
}

fn extract_unique_id_near_sec_uid(dat_bytes: &[u8], sec_uid: &str) -> Option<String> {
    let sec_uid = sec_uid.trim();
    if sec_uid.is_empty() {
        return None;
    }

    let needle = sec_uid.as_bytes();
    let mut search_start = 0usize;
    while let Some(relative) = find_bytes(dat_bytes.get(search_start..).unwrap_or(&[]), needle) {
        let sec_pos = search_start + relative;
        let window_start = sec_pos.saturating_sub(32);
        let prefix = &dat_bytes[window_start..sec_pos];

        let mut i = 0usize;
        let mut last_candidate = None;
        while i < prefix.len() {
            if prefix[i].is_ascii_digit() {
                let start = i;
                while i < prefix.len() && prefix[i].is_ascii_digit() {
                    i += 1;
                }
                let end = i;
                let len = end - start;
                let gap = prefix.len().saturating_sub(end);
                if (5..=20).contains(&len) && gap <= 4 {
                    last_candidate = std::str::from_utf8(&prefix[start..end])
                        .ok()
                        .map(|value| value.to_string());
                }
            } else {
                i += 1;
            }
        }

        if let Some(candidate) = last_candidate {
            return Some(candidate);
        }

        search_start = sec_pos.saturating_add(needle.len());
    }

    None
}

fn format_douyin_auth_time_label(value: f64) -> String {
    if !value.is_finite() || value <= 0.0 {
        return String::new();
    }
    let whole_seconds = value.floor();
    let fractional = value - whole_seconds;
    let nanos = (fractional * 1_000_000_000.0).round();
    let whole_seconds = whole_seconds as u64;
    let nanos = nanos.clamp(0.0, 999_999_999.0) as u32;
    format!("{whole_seconds}.{nanos:09}")
}

fn read_douyin_local_account_payload(zip_path: &str) -> Result<Option<Value>, String> {
    let plist_path = find_app_file_path(
        zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/Preferences/com.ss.iphone.ugc.Aweme.plist"],
    )?;
    let Some(plist_path) = plist_path else {
        return Ok(None);
    };

    let plist_bytes = read_zip_entry_bytes(zip_path, &plist_path)?;
    let plist_value = plist::Value::from_reader(Cursor::new(plist_bytes.as_slice()))
        .map_err(|error| format!("douyin_local_account_parse_failed: {error}"))?;
    extract_douyin_local_account_payload(&plist_value)
}

/// Read TTAccountSDKUserInfo.archiver (and loginData.dat as fallback) from the
/// backup zip and extract third-party platform connects (QQ / Google / WeChat /
/// Apple / Toutiao) for each known user. This acts as a local fallback when the
/// session API is unreachable (e.g. token expired or account logged out).
fn read_douyin_sdk_user_info_connects(zip_path: &str) -> Result<Vec<(String, Value)>, String> {
    let mut by_uid: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    // Primary source: ttaccountSDKUserInfo.archiver
    if let Some(archiver_path) = find_app_file_path(
        zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Documents/ttaccountSDKUserInfo.archiver"],
    )? {
        let bytes = read_zip_entry_bytes(zip_path, &archiver_path)?;
        if let Some(root) = decode_nskeyed_archive_payload(&bytes)? {
            for (uid, connects) in group_sdk_user_info_connects_by_uid(&root) {
                by_uid.entry(uid).or_default().extend(connects);
            }
        }
    }

    // Secondary source: loginData.dat (strings-based extraction fallback)
    if let Some(dat_path) = find_app_file_path(
        zip_path,
        "com.ss.iphone.ugc.Aweme",
        &["Library/loginData.dat"],
    )? {
        let dat_bytes = read_zip_entry_bytes(zip_path, &dat_path)?;
        let login_data_connects = crate::extract_login_data_connects(&dat_bytes);
        for (uid, connect) in login_data_connects {
            by_uid.entry(uid).or_default().push(connect);
        }
    }

    // Wrap each uid's connects in the {"connects": [...]} shape that
    // parse_douyin_session_bindings_for_uid expects.
    let result: Vec<(String, Value)> = by_uid
        .into_iter()
        .map(|(uid, connects)| {
            let payload = serde_json::json!({"connects": connects});
            (uid, payload)
        })
        .collect();

    Ok(result)
}

fn group_sdk_user_info_connects_by_uid(root: &Value) -> BTreeMap<String, Vec<Value>> {
    let mut by_uid: BTreeMap<String, Vec<Value>> = BTreeMap::new();

    if let Some(connects) = root.get("connects").and_then(Value::as_array) {
        for connect in connects {
            let Some(uid) = douyin_connect_owner_uid(connect) else {
                continue;
            };
            by_uid.entry(uid).or_default().push(connect.clone());
        }
    }

    by_uid
}

/// Extract QQ / Google platform connects from loginData.dat using raw-byte
/// heuristics. The archive is frequently malformed, so connect markers are
/// scanned directly from `dat_bytes` instead of a lossy UTF-8 projection.
fn extract_login_data_connects(dat_bytes: &[u8]) -> Vec<(String, Value)> {
    if dat_bytes.is_empty() {
        return Vec::new();
    }

    // Douyin user IDs are still safe to locate from lossy text because the
    // numeric runs themselves survive replacement unchanged.
    let text = String::from_utf8_lossy(dat_bytes);
    let user_ids = map_lossy_user_ids_to_raw_positions(dat_bytes, find_numeric_ids(&text, 15, 20));
    if user_ids.is_empty() {
        return Vec::new();
    }

    let mut markers = find_qq_markers(dat_bytes);
    markers.extend(find_google_markers(dat_bytes));
    if markers.is_empty() {
        return Vec::new();
    }

    // Map each marker to the nearest raw-byte user ID position.
    let max_dist = 200_000usize;
    let mut by_uid: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();

    for (marker_pos, kind, value) in markers {
        let Some((uid_str, uid_pos)) = user_ids
            .iter()
            .min_by_key(|(_, user_pos)| marker_pos.abs_diff(*user_pos))
        else {
            continue;
        };

        if marker_pos.abs_diff(*uid_pos) > max_dist {
            continue;
        }

        let connect = match kind {
            "qzone_sns" => serde_json::json!({"platform": "qzone_sns", "platformUID": value}),
            "google" => serde_json::json!({"platform": "google", "platformUID": value}),
            _ => continue,
        };
        let connects = by_uid.entry(uid_str.clone()).or_default();
        if !connects.iter().any(|existing| {
            existing.get("platform") == connect.get("platform")
                && existing.get("platformUID") == connect.get("platformUID")
        }) {
            connects.push(connect);
        }
    }

    // Attach Google screen names
    for (uid_str, uid_pos) in &user_ids {
        if let Some(connects) = by_uid.get_mut(uid_str) {
            let ctx_start = uid_pos.saturating_sub(8192);
            let ctx_end = (uid_pos + uid_str.len() + 8192).min(dat_bytes.len());
            let ctx = String::from_utf8_lossy(&dat_bytes[ctx_start..ctx_end]);
            if let Some(sn) = find_google_screen_name(&ctx) {
                for c in connects.iter_mut() {
                    if let Some(obj) = c.as_object_mut() {
                        if obj.get("platform").and_then(Value::as_str) == Some("google") {
                            obj.entry("platformScreenName".to_string())
                                .or_insert(Value::String(sn.clone()));
                        }
                    }
                }
            }
        }
    }

    by_uid
        .into_iter()
        .flat_map(|(uid, connects)| connects.into_iter().map(move |c| (uid.clone(), c)))
        .collect()
}

/// Find all numeric strings of length [min_len, max_len] in `text`, returning
/// (id_string, byte_offset) pairs.
/// Find a Google-style two-word screen name (e.g. "uepx juvx") in the
/// context text by scanning for letter sequences that contain a space.
fn find_google_screen_name(ctx: &str) -> Option<String> {
    let bytes = ctx.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b' ') {
            i += 1;
        }
        let candidate = &ctx[start..i];
        let trimmed = candidate.trim();
        if trimmed.contains(' ') && trimmed.len() >= 6 && trimmed.len() <= 40 {
            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
            if parts.len() == 2
                && parts[0].len() >= 2
                && parts[1].len() >= 2
                && parts[0].chars().all(|c: char| c.is_ascii_alphabetic())
                && parts[1].chars().all(|c: char| c.is_ascii_alphabetic())
            {
                let lowered = trimmed.to_lowercase();
                if !lowered.contains("error")
                    && !lowered.contains("account")
                    && !lowered.contains("google")
                    && !lowered.contains("apple")
                    && !lowered.contains("request")
                    && !lowered.contains("trigger")
                    && !lowered.contains("screen")
                {
                    return Some(clean_login_data_screen_name(trimmed));
                }
            }
        }
    }
    None
}

fn clean_login_data_screen_name(value: &str) -> String {
    let mut parts = value.split_whitespace();
    let Some(first) = parts.next() else {
        return value.trim().to_string();
    };
    let rest = parts.collect::<Vec<_>>();
    if rest.is_empty() {
        return value.trim().to_string();
    }

    let cleaned_first = clean_login_data_screen_name_token(first);
    let mut rebuilt = Vec::with_capacity(rest.len() + 1);
    rebuilt.push(cleaned_first);
    rebuilt.extend(rest.into_iter().map(str::to_string));
    rebuilt.join(" ")
}

fn clean_login_data_screen_name_token(token: &str) -> String {
    if token.chars().all(|ch| ch.is_ascii_lowercase()) {
        return token.to_string();
    }

    let mut last_upper_boundary = None;
    for (index, ch) in token.char_indices() {
        if ch.is_ascii_uppercase() {
            last_upper_boundary = Some(index + ch.len_utf8());
        }
    }

    let Some(boundary) = last_upper_boundary else {
        return token.to_string();
    };
    let suffix = token.get(boundary..).unwrap_or_default();
    if suffix.len() >= 2 && suffix.chars().all(|ch| ch.is_ascii_lowercase()) {
        suffix.to_string()
    } else {
        token.to_string()
    }
}

fn find_numeric_ids(text: &str, min_len: usize, max_len: usize) -> Vec<(&str, usize)> {
    let mut ids = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            let len = i - start;
            if len >= min_len && len <= max_len {
                let candidate = &text[start..i];
                if is_plausible_login_data_user_id(candidate) {
                    ids.push((candidate, start));
                }
            }
        } else {
            i += 1;
        }
    }
    ids
}

fn is_plausible_login_data_user_id(value: &str) -> bool {
    !looks_like_compact_datetime_id(value)
}

fn looks_like_compact_datetime_id(value: &str) -> bool {
    if !matches!(value.len(), 15..=17) || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }

    let parse = |start: usize, end: usize| {
        value
            .get(start..end)
            .and_then(|part| part.parse::<u32>().ok())
    };
    let Some(year) = parse(0, 4) else {
        return false;
    };
    let Some(month) = parse(4, 6) else {
        return false;
    };
    let Some(day) = parse(6, 8) else {
        return false;
    };
    let Some(hour) = parse(8, 10) else {
        return false;
    };
    let Some(minute) = parse(10, 12) else {
        return false;
    };
    let Some(second) = parse(12, 14) else {
        return false;
    };

    (2010..=2099).contains(&year)
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour <= 23
        && minute <= 59
        && second <= 59
}

fn map_lossy_user_ids_to_raw_positions(
    dat_bytes: &[u8],
    lossy_ids: Vec<(&str, usize)>,
) -> Vec<(String, usize)> {
    let mut mapped = Vec::new();
    let mut search_start = 0usize;

    for (uid, _) in lossy_ids {
        let uid_bytes = uid.as_bytes();
        let next_pos = find_bytes(dat_bytes.get(search_start..).unwrap_or(&[]), uid_bytes)
            .map(|relative| search_start + relative)
            .or_else(|| find_bytes(dat_bytes, uid_bytes));

        if let Some(raw_pos) = next_pos {
            mapped.push((uid.to_string(), raw_pos));
            search_start = raw_pos.saturating_add(uid_bytes.len());
        }
    }

    mapped
}

fn find_qq_markers(dat_bytes: &[u8]) -> Vec<(usize, &'static str, String)> {
    let mut markers = Vec::new();
    let mut offset = 0usize;

    while let Some(relative) = find_bytes(dat_bytes.get(offset..).unwrap_or(&[]), b"UID_") {
        let marker_pos = offset + relative;
        let hex_start = marker_pos + 4;
        let Some(hex_end) = hex_start.checked_add(32) else {
            break;
        };
        if hex_end <= dat_bytes.len() {
            let hex_bytes = &dat_bytes[hex_start..hex_end];
            if hex_bytes.iter().all(u8::is_ascii_hexdigit) {
                let tail_end = (hex_end + 200).min(dat_bytes.len());
                if find_bytes(&dat_bytes[hex_end..tail_end], b"qzone_sns").is_some() {
                    if let Ok(hex) = std::str::from_utf8(hex_bytes) {
                        markers.push((marker_pos, "qzone_sns", format!("UID_{hex}")));
                    }
                }
            }
        }
        offset = marker_pos + 4;
    }

    markers
}

fn find_google_markers(dat_bytes: &[u8]) -> Vec<(usize, &'static str, String)> {
    let mut markers = Vec::new();
    let mut offset = 0usize;

    while let Some(relative) = find_bytes(dat_bytes.get(offset..).unwrap_or(&[]), b"\x5f\x10\x15") {
        let prefix_pos = offset + relative;
        if let Some((digits_start, digits)) =
            extract_ascii_digits_after(dat_bytes, prefix_pos + 3, 15, 30)
        {
            markers.push((digits_start, "google", digits));
        }
        offset = prefix_pos + 3;
    }

    offset = 0;
    while let Some(relative) = find_bytes(dat_bytes.get(offset..).unwrap_or(&[]), b"Vgoogle") {
        let google_pos = offset + relative;
        let mut digits_start = google_pos;
        while digits_start > 0 && dat_bytes[digits_start - 1].is_ascii_digit() {
            digits_start -= 1;
        }
        let digits_len = google_pos - digits_start;
        if (15..=30).contains(&digits_len) {
            if let Ok(digits) = std::str::from_utf8(&dat_bytes[digits_start..google_pos]) {
                markers.push((digits_start, "google", digits.to_string()));
            }
        }
        offset = google_pos + b"Vgoogle".len();
    }

    markers
}

fn extract_ascii_digits_after(
    dat_bytes: &[u8],
    start: usize,
    min_len: usize,
    max_len: usize,
) -> Option<(usize, String)> {
    if start >= dat_bytes.len() || !dat_bytes[start].is_ascii_digit() {
        return None;
    }

    let mut end = start;
    while end < dat_bytes.len() && dat_bytes[end].is_ascii_digit() {
        end += 1;
    }

    let digits_len = end - start;
    if !(min_len..=max_len).contains(&digits_len) {
        return None;
    }

    std::str::from_utf8(&dat_bytes[start..end])
        .ok()
        .map(|digits| (start, digits.to_string()))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn read_douyin_local_account_payload_from_plist_value(
    plist_value: &plist::Value,
    key: &str,
) -> Result<Option<Value>, String> {
    let Some(dict) = plist_value.as_dictionary() else {
        return Ok(None);
    };
    let Some(raw_data) = dict.get(key).and_then(plist::Value::as_data) else {
        return Ok(None);
    };
    let archive_bytes = decode_douyin_local_account_archive_bytes(raw_data)?;
    decode_nskeyed_archive_payload(&archive_bytes)
}

fn extract_douyin_local_account_payload(
    plist_value: &plist::Value,
) -> Result<Option<Value>, String> {
    let Some(dict) = plist_value.as_dictionary() else {
        return Ok(None);
    };

    for key in [
        "kDYACurrentLoginUserPersistenceKey",
        "kDYAAllLoginUserPersistenceKey",
    ] {
        let Some(raw_data) = dict.get(key).and_then(plist::Value::as_data) else {
            continue;
        };
        let archive_bytes = decode_douyin_local_account_archive_bytes(raw_data)?;
        if let Some(payload) = decode_nskeyed_archive_payload(&archive_bytes)? {
            return Ok(Some(payload));
        }
    }

    Ok(None)
}

fn parse_douyin_local_account_identities(payload: &Value) -> Vec<DouyinLocalAccountIdentity> {
    let Some(items) = payload.get("NS.objects").and_then(Value::as_array) else {
        return parse_douyin_local_account_identity_from_payload(payload)
            .into_iter()
            .collect();
    };

    let mut seen = BTreeMap::new();
    let mut accounts = Vec::new();
    for item in items {
        let Some(mut identity) = parse_douyin_local_account_identity_from_payload(item) else {
            continue;
        };
        identity = merge_root_connects_into_local_account(identity, payload);
        if identity.uid.is_empty() || seen.contains_key(&identity.uid) {
            continue;
        }
        seen.insert(identity.uid.clone(), true);
        accounts.push(identity);
    }

    accounts
}

fn merge_root_connects_into_local_account(
    mut identity: DouyinLocalAccountIdentity,
    root_payload: &Value,
) -> DouyinLocalAccountIdentity {
    if identity.uid.trim().is_empty() {
        return identity;
    }

    let root_bindings = parse_douyin_session_bindings_for_uid(root_payload, Some(&identity.uid));
    if root_bindings.summary.is_empty() {
        return identity;
    }

    identity.bindings = merge_douyin_session_bindings(identity.bindings, root_bindings);
    identity
}

fn parse_douyin_local_account_identity_from_payload(
    payload: &Value,
) -> Option<DouyinLocalAccountIdentity> {
    let aweme_account = payload.get("awemeAccount").unwrap_or(payload);
    let raw_user = aweme_account
        .get("rawData")
        .and_then(|value| value.get("user"))
        .or_else(|| payload.get("rawData").and_then(|value| value.get("user")));

    let uid = first_non_empty_strings(&[
        aweme_account
            .get("userID")
            .and_then(douyin_normalize_json_value),
        raw_user
            .and_then(|value| value.get("uid"))
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default();
    if uid.is_empty() {
        return None;
    }

    let nickname = first_non_empty_strings(&[
        aweme_account
            .get("nickname")
            .and_then(douyin_normalize_json_value),
        raw_user
            .and_then(|value| value.get("nickname"))
            .and_then(douyin_normalize_json_value),
        raw_user
            .and_then(|value| value.get("other_nickname"))
            .and_then(douyin_normalize_json_value),
    ])
    .unwrap_or_default();
    let sec_uid = raw_user
        .and_then(|value| value.get("sec_uid"))
        .and_then(douyin_normalize_json_value)
        .unwrap_or_default();
    let unique_id = raw_user
        .and_then(|value| value.get("unique_id"))
        .and_then(douyin_normalize_json_value)
        .unwrap_or_default();
    let short_id = raw_user
        .and_then(|value| value.get("short_id"))
        .and_then(douyin_normalize_json_value)
        .unwrap_or_default();
    let parsed_password = parse_douyin_password_status_payload(payload);
    let parsed_certification = parse_douyin_certification_status_payload(payload);
    let parsed_token = parse_douyin_token_check_payload(payload);
    let phone_number = prefer_better_phone_number(
        parse_douyin_mobile_change_payload(payload),
        parsed_token.phone_number.clone(),
    )
    .unwrap_or_default();
    let register_time = first_non_empty_strings(&[
        parsed_password.register_time.clone(),
        parsed_token.register_time.clone(),
    ])
    .unwrap_or_default();
    let mut normal_functions = parsed_token
        .functions
        .into_iter()
        .filter(|item| item.func_available)
        .map(|item| item.func_name)
        .collect::<Vec<_>>();
    if parsed_password.has_password == Some(true) {
        normal_functions.push("改密功能".to_string());
    }
    if parsed_certification.is_verified == Some(true) {
        normal_functions.push("实名正常".to_string());
    }
    normal_functions = dedupe_owned_text(normal_functions);

    Some(DouyinLocalAccountIdentity {
        uid,
        nickname: first_non_empty_strings(&[
            Some(nickname),
            parsed_password.screen_name.clone(),
            parsed_token.nickname.clone(),
        ])
        .unwrap_or_default(),
        sec_uid,
        unique_id,
        short_id,
        phone_number,
        register_time,
        aweme_count: parsed_token.aweme_count.unwrap_or_default(),
        following_count: parsed_token.following_count.unwrap_or_default(),
        liked_count: parsed_token.liked_count.unwrap_or_default(),
        bindings: parsed_password.bindings,
        has_password: parsed_password.has_password,
        is_verified: parsed_certification.is_verified,
        normal_functions,
    })
}

fn decode_douyin_local_account_archive_bytes(raw_data: &[u8]) -> Result<Vec<u8>, String> {
    if raw_data.starts_with(b"bplist00") {
        return Ok(raw_data.to_vec());
    }

    let encoded = std::str::from_utf8(raw_data)
        .map_err(|error| format!("douyin_local_account_parse_failed: {error}"))?
        .trim();
    BASE64_STANDARD
        .decode(encoded)
        .map_err(|error| format!("douyin_local_account_parse_failed: {error}"))
}

fn decode_nskeyed_archive_payload(bytes: &[u8]) -> Result<Option<Value>, String> {
    let archive = plist::Value::from_reader(Cursor::new(bytes))
        .map_err(|error| format!("douyin_local_account_parse_failed: {error}"))?;
    let Some(dict) = archive.as_dictionary() else {
        return Ok(None);
    };
    let Some(objects) = dict.get("$objects").and_then(plist::Value::as_array) else {
        return Ok(None);
    };
    let Some(top) = dict.get("$top").and_then(plist::Value::as_dictionary) else {
        return Ok(None);
    };
    let Some(root_uid) = top
        .get("root")
        .and_then(plist::Value::as_uid)
        .or_else(|| top.values().find_map(plist::Value::as_uid))
    else {
        return Ok(None);
    };

    let mut stack = Vec::new();
    decode_nskeyed_archive_value(&plist::Value::Uid(*root_uid), objects, &mut stack).map(Some)
}

fn decode_nskeyed_archive_value(
    value: &plist::Value,
    objects: &[plist::Value],
    stack: &mut Vec<usize>,
) -> Result<Value, String> {
    match value {
        plist::Value::Uid(uid) => {
            let index = uid.get() as usize;
            if stack.contains(&index) {
                return Ok(Value::String(format!("<cycle:{index}>")));
            }
            let Some(object) = objects.get(index) else {
                return Err(format!(
                    "douyin_local_account_parse_failed: missing_uid_{index}"
                ));
            };
            stack.push(index);
            let decoded = decode_nskeyed_archive_value(object, objects, stack);
            stack.pop();
            decoded
        }
        plist::Value::Array(values) => values
            .iter()
            .map(|item| decode_nskeyed_archive_value(item, objects, stack))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        plist::Value::Dictionary(dict) => {
            if let (Some(keys), Some(values)) = (
                dict.get("NS.keys").and_then(plist::Value::as_array),
                dict.get("NS.objects").and_then(plist::Value::as_array),
            ) {
                let mut object = Map::new();
                for (key, value) in keys.iter().zip(values.iter()) {
                    let decoded_key = decode_nskeyed_archive_value(key, objects, stack)?;
                    let key_text = match decoded_key {
                        Value::String(text) => text,
                        Value::Number(number) => number.to_string(),
                        Value::Bool(flag) => flag.to_string(),
                        _ => continue,
                    };
                    object.insert(
                        key_text,
                        decode_nskeyed_archive_value(value, objects, stack)?,
                    );
                }
                return Ok(Value::Object(object));
            }

            let mut object = Map::new();
            for (key, nested) in dict {
                if matches!(key.as_str(), "$class" | "$classes" | "$classname") {
                    continue;
                }
                object.insert(
                    key.clone(),
                    decode_nskeyed_archive_value(nested, objects, stack)?,
                );
            }
            Ok(Value::Object(object))
        }
        _ => serde_json::to_value(value.clone())
            .map_err(|error| format!("douyin_local_account_parse_failed: {error}")),
    }
}

fn parse_douyin_mobile_change_payload(payload: &Value) -> Option<String> {
    let direct_phone_number = json_strings_from_paths(
        payload,
        &[
            &["new_mobile_info", "newMobile"],
            &["new_mobile_info", "lastNewMobile"],
            &["passportAccount", "phoneNumber"],
            &["passportAccount", "phone_number"],
            &["awemeAccount", "phoneNumber"],
            &["awemeAccount", "phone_number"],
            &["localPropertiesStorage", "phoneNumber"],
            &["localPropertiesStorage", "phone_number"],
            &["phoneNumber"],
            &["newMobile"],
            &["businessModel", "bindPhone"],
        ],
    )
    .into_iter()
    .fold(None, |best, candidate| {
        prefer_better_phone_number(best, Some(candidate))
    });

    let fallback_phone_number = first_json_string_from_paths(
        payload,
        &[
            &["new_mobile_info", "newPhone"],
            &["passportAccount", "rawData", "data", "mobile"],
            &["newPhone"],
        ],
    )
    .filter(|value| !value.trim().is_empty())
    .map(|value| value.trim().to_string())
    .map(|phone_number| {
        if phone_number.starts_with('+') {
            return phone_number;
        }

        let country_code = first_json_string_from_paths(
            payload,
            &[
                &["new_mobile_info", "country_code"],
                &["passportAccount", "rawData", "data", "country_code"],
                &["passportAccount", "countryCode"],
                &["passportAccount", "safeMobileCountryCode"],
                &["businessModel", "countryCode"],
            ],
        )
        .and_then(|value| normalize_phone_country_code(&value));

        match country_code {
            Some(prefix) => format!("{prefix} {}", phone_number),
            None => phone_number,
        }
    });

    prefer_better_phone_number(direct_phone_number, fallback_phone_number)
}

fn normalize_phone_country_code(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.is_empty() || normalized == "0" {
        return None;
    }
    if normalized.starts_with('+') {
        return Some(normalized.to_string());
    }
    if normalized.chars().all(|char| char.is_ascii_digit()) {
        return Some(format!("+{normalized}"));
    }
    None
}

fn parse_douyin_token_check_payload(payload: &Value) -> ParsedDouyinTokenCheck {
    let status_code = payload
        .get("status_code")
        .and_then(Value::as_i64)
        .or_else(|| payload.get("status").and_then(Value::as_i64));
    let message = first_non_empty_strings(&[
        payload
            .get("status_msg")
            .and_then(douyin_normalize_json_value),
        payload.get("message").and_then(douyin_normalize_json_value),
        payload
            .get("error_msg")
            .and_then(douyin_normalize_json_value),
        douyin_json_value(payload, &["data", "description"]).and_then(douyin_normalize_json_value),
    ]);
    let uid = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "userID"],
            &["awemeAccount", "rawData", "user", "uid"],
            &["rawData", "user", "uid"],
            &["user", "uid"],
            &["data", "user", "uid"],
            &["data", "uid"],
            &["uid"],
        ],
    );
    let sec_uid = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "rawData", "user", "sec_uid"],
            &["rawData", "user", "sec_uid"],
            &["user", "sec_uid"],
            &["data", "user", "sec_uid"],
            &["data", "sec_uid"],
            &["sec_uid"],
        ],
    );
    let nickname = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "nickname"],
            &["awemeAccount", "rawData", "user", "nickname"],
            &["rawData", "user", "nickname"],
            &["passportAccount", "screenName"],
            &["passportAccount", "screen_name"],
            &["user", "nickname"],
            &["data", "user", "nickname"],
            &["data", "nickname"],
            &["nickname"],
        ],
    );
    let phone_number = first_json_string_from_paths(
        payload,
        &[
            &["passportAccount", "phoneNumber"],
            &["passportAccount", "phone_number"],
            &["awemeAccount", "phoneNumber"],
            &["awemeAccount", "phone_number"],
            &["passportAccount", "rawData", "data", "mobile"],
            &["businessModel", "bindPhone"],
            &["user", "mobile"],
            &["user", "phone"],
            &["user", "phone_number"],
            &["user", "bind_phone"],
            &["data", "user", "mobile"],
            &["data", "user", "phone"],
            &["data", "mobile"],
            &["data", "phone"],
            &["mobile"],
            &["phone"],
        ],
    );
    let register_time = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "rawData", "user", "create_time"],
            &["awemeAccount", "rawData", "user", "register_time"],
            &["awemeAccount", "rawData", "user", "user_create_time"],
            &["passportAccount", "user_create_time"],
            &["passportAccount", "rawData", "data", "create_time"],
            &["passportAccount", "rawData", "data", "register_time"],
            &["passportAccount", "rawData", "data", "user_create_time"],
            &["user", "create_time"],
            &["user", "register_time"],
            &["user", "user_create_time"],
            &["user", "account_create_time"],
            &["data", "user", "create_time"],
            &["data", "user", "register_time"],
            &["data", "user", "user_create_time"],
            &["data", "create_time"],
            &["data", "register_time"],
            &["data", "user_create_time"],
            &["create_time"],
            &["register_time"],
            &["user_create_time"],
        ],
    );
    let aweme_count = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "rawData", "user", "aweme_count"],
            &["user", "aweme_count"],
            &["data", "user", "aweme_count"],
            &["data", "aweme_count"],
            &["aweme_count"],
        ],
    );
    let following_count = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "rawData", "user", "following_count"],
            &["user", "following_count"],
            &["data", "user", "following_count"],
            &["data", "following_count"],
            &["following_count"],
        ],
    );
    let liked_count = first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "rawData", "user", "total_favorited"],
            &["awemeAccount", "rawData", "user", "favoriting_count"],
            &["awemeAccount", "rawData", "user", "liked_count"],
            &["user", "total_favorited"],
            &["user", "favoriting_count"],
            &["user", "liked_count"],
            &["data", "user", "total_favorited"],
            &["data", "total_favorited"],
            &["total_favorited"],
        ],
    );
    let has_identity = uid.is_some() || sec_uid.is_some() || nickname.is_some();
    let has_data = payload
        .get("data")
        .map(|value| !value.is_null())
        .unwrap_or(false);
    let is_valid = match status_code {
        Some(0) => Some(true),
        Some(_) => Some(false),
        None if has_identity => Some(true),
        None if has_data && message.as_deref() == Some("success") => Some(true),
        None => None,
    };

    let functions = parse_douyin_function_items(payload);

    ParsedDouyinTokenCheck {
        is_valid,
        status_code,
        message,
        uid,
        sec_uid,
        nickname,
        phone_number,
        register_time,
        aweme_count,
        following_count,
        liked_count,
        functions,
    }
}

fn parse_douyin_function_items(payload: &Value) -> Vec<DouyinFunctionItem> {
    let data = payload.get("data").or(Some(payload));
    let list = data.and_then(|d| {
        d.get("function_list")
            .or_else(|| d.get("func_list"))
            .or_else(|| d.get("func_elements"))
            .or_else(|| d.get("functions"))
            .or_else(|| d.get("func_list_v2"))
            .and_then(Value::as_array)
    });
    let Some(list) = list else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| {
            let func_name = item
                .get("func_name")
                .or_else(|| item.get("function_name"))
                .or_else(|| item.get("name"))
                .and_then(Value::as_str)
                .map(|s| s.to_string())?;
            let func_available = item
                .get("func_available")
                .or_else(|| item.get("func_avaliable"))
                .or_else(|| item.get("available"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Some(DouyinFunctionItem {
                func_name,
                func_available,
            })
        })
        .collect()
}

fn parse_douyin_profile_other_payload(
    payload: &Value,
    fallback_sec_uid: &str,
) -> ParsedDouyinProfileOtherIdentity {
    let uid = first_json_string_from_paths(
        payload,
        &[
            &["user", "uid"],
            &["data", "user", "uid"],
            &["data", "uid"],
            &["uid"],
        ],
    );
    let sec_uid = first_json_string_from_paths(
        payload,
        &[
            &["user", "sec_uid"],
            &["data", "user", "sec_uid"],
            &["data", "sec_uid"],
            &["sec_uid"],
        ],
    )
    .or_else(|| {
        let trimmed = fallback_sec_uid.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    let unique_id = first_json_string_from_paths(
        payload,
        &[
            &["user", "unique_id"],
            &["data", "user", "unique_id"],
            &["data", "unique_id"],
            &["unique_id"],
        ],
    );
    ParsedDouyinProfileOtherIdentity {
        uid,
        sec_uid,
        unique_id,
    }
}

fn first_json_string_from_paths(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .find_map(|path| douyin_json_value(value, path).and_then(douyin_normalize_json_value))
        .filter(|value| !value.is_empty())
}

fn json_strings_from_paths(value: &Value, paths: &[&[&str]]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| douyin_json_value(value, path).and_then(douyin_normalize_json_value))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn prefer_better_phone_number(
    current: Option<String>,
    candidate: Option<String>,
) -> Option<String> {
    match (current, candidate) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value),
        (Some(current), Some(candidate)) => {
            if phone_number_quality_score(&candidate) > phone_number_quality_score(&current) {
                Some(candidate)
            } else {
                Some(current)
            }
        }
    }
}

fn phone_number_quality_score(value: &str) -> isize {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0;
    }

    let digits = trimmed.chars().filter(|char| char.is_ascii_digit()).count() as isize;
    let has_country_prefix = if trimmed.starts_with('+') { 1 } else { 0 };
    let is_masked = if trimmed.contains('*') { 1 } else { 0 };

    digits * 10 + has_country_prefix * 3 - is_masked * 100
}

fn mask_secret(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars = trimmed.chars().collect::<Vec<_>>();
    if chars.len() <= 14 {
        return "***".to_string();
    }
    let head = chars.iter().take(4).collect::<String>();
    let tail = chars
        .iter()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{head}...{tail}")
}

fn first_non_empty_strings(values: &[Option<String>]) -> Option<String> {
    values
        .iter()
        .flatten()
        .find(|value| !value.is_empty())
        .cloned()
}

fn parse_douyin_screen_name(payload: &Value) -> Option<String> {
    let data = payload.get("data").unwrap_or(payload);
    let user = data.get("user");
    let passport_account = payload
        .get("passportAccount")
        .or_else(|| data.get("passportAccount"));
    let app_user_info = passport_account
        .and_then(|value| value.get("appUserInfo"))
        .or_else(|| payload.get("appUserInfo"))
        .or_else(|| data.get("appUserInfo"));
    let aweme_account = payload
        .get("awemeAccount")
        .or_else(|| data.get("awemeAccount"));
    let aweme_raw_user = aweme_account
        .and_then(|value| value.get("rawData"))
        .and_then(|value| value.get("user"));
    let business_model = payload
        .get("businessModel")
        .or_else(|| data.get("businessModel"));
    let ticket_model = passport_account.and_then(|value| value.get("ticketModel"));

    first_non_empty_strings(&[
        user.and_then(|u| u.get("name"))
            .and_then(douyin_normalize_json_value),
        data.get("name").and_then(douyin_normalize_json_value),
        user.and_then(|u| u.get("screen_name"))
            .and_then(douyin_normalize_json_value),
        data.get("screen_name")
            .and_then(douyin_normalize_json_value),
        user.and_then(|u| u.get("screenName"))
            .and_then(douyin_normalize_json_value),
        data.get("screenName").and_then(douyin_normalize_json_value),
        passport_account
            .and_then(|value| value.get("screen_name"))
            .and_then(douyin_normalize_json_value),
        passport_account
            .and_then(|value| value.get("screenName"))
            .and_then(douyin_normalize_json_value),
        passport_account
            .and_then(|value| value.get("name"))
            .and_then(douyin_normalize_json_value),
        app_user_info
            .and_then(|value| value.get("screen_name"))
            .and_then(douyin_normalize_json_value),
        app_user_info
            .and_then(|value| value.get("screenName"))
            .and_then(douyin_normalize_json_value),
        app_user_info
            .and_then(|value| value.get("name"))
            .and_then(douyin_normalize_json_value),
        aweme_account
            .and_then(|value| value.get("nickname"))
            .and_then(douyin_normalize_json_value),
        aweme_raw_user
            .and_then(|value| value.get("nickname"))
            .and_then(douyin_normalize_json_value),
        aweme_raw_user
            .and_then(|value| value.get("other_nickname"))
            .and_then(douyin_normalize_json_value),
        business_model
            .and_then(|value| value.get("nickname"))
            .and_then(douyin_normalize_json_value),
        ticket_model
            .and_then(|value| value.get("nickName"))
            .and_then(douyin_normalize_json_value),
    ])
}

fn douyin_connects_array(payload: &Value) -> Option<&Vec<Value>> {
    payload
        .get("data")
        .and_then(|value| value.get("connects"))
        .or_else(|| payload.get("connects"))
        .and_then(Value::as_array)
}

fn parse_douyin_session_bindings(payload: &Value) -> DouyinSessionBindings {
    let target_uid = douyin_payload_uid(payload);
    parse_douyin_session_bindings_for_uid(payload, target_uid.as_deref())
}

fn parse_douyin_session_bindings_for_uid(
    payload: &Value,
    target_uid: Option<&str>,
) -> DouyinSessionBindings {
    let mut bindings = DouyinSessionBindings::default();
    let Some(connects) = douyin_connects_array(payload) else {
        return bindings;
    };

    for connect in connects {
        if !should_include_douyin_connect(connect, target_uid) {
            continue;
        }
        let platform = connect
            .get("platform")
            .and_then(douyin_normalize_json_value)
            .unwrap_or_default();
        let Some(binding_platform) = normalize_douyin_binding_platform(&platform) else {
            continue;
        };
        let binding_value = format_douyin_connect_binding_value(connect);
        let screen_name = douyin_connect_screen_name(connect).unwrap_or_default();
        if binding_value.is_empty() && screen_name.is_empty() {
            continue;
        }

        match binding_platform {
            DouyinSessionBindingPlatform::Toutiao => merge_douyin_binding_slot(
                &mut bindings.toutiao,
                &mut bindings.toutiao_platform_screen_name,
                binding_value,
                screen_name,
            ),
            DouyinSessionBindingPlatform::Qq => merge_douyin_binding_slot(
                &mut bindings.qq,
                &mut bindings.qq_platform_screen_name,
                binding_value,
                screen_name,
            ),
            DouyinSessionBindingPlatform::Google => merge_douyin_binding_slot(
                &mut bindings.google,
                &mut bindings.google_platform_screen_name,
                binding_value,
                screen_name,
            ),
            DouyinSessionBindingPlatform::AppleId => merge_douyin_binding_slot(
                &mut bindings.apple_id,
                &mut bindings.apple_id_platform_screen_name,
                binding_value,
                screen_name,
            ),
            DouyinSessionBindingPlatform::Wechat => merge_douyin_binding_slot(
                &mut bindings.wechat,
                &mut bindings.wechat_platform_screen_name,
                binding_value,
                screen_name,
            ),
        }
    }

    bindings.summary = build_douyin_session_bindings_summary(&bindings);
    bindings
}

fn should_include_douyin_connect(connect: &Value, target_uid: Option<&str>) -> bool {
    let Some(target_uid) = target_uid.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let Some(connect_uid) = douyin_connect_owner_uid(connect) else {
        return true;
    };

    connect_uid == target_uid
}

fn douyin_connect_owner_uid(connect: &Value) -> Option<String> {
    first_non_empty_strings(&[
        connect.get("user_id").and_then(douyin_normalize_json_value),
        connect.get("userID").and_then(douyin_normalize_json_value),
        connect.get("uid").and_then(douyin_normalize_json_value),
    ])
}

fn douyin_payload_uid(payload: &Value) -> Option<String> {
    first_json_string_from_paths(
        payload,
        &[
            &["awemeAccount", "userID"],
            &["awemeAccount", "rawData", "user", "uid"],
            &["rawData", "user", "uid"],
            &["user", "uid"],
            &["data", "user", "uid"],
            &["data", "uid"],
            &["uid"],
        ],
    )
}

fn build_douyin_session_bindings_summary(bindings: &DouyinSessionBindings) -> String {
    let mut labels = Vec::new();
    for (label, value, screen_name) in [
        (
            DouyinSessionBindingPlatform::Toutiao.label(),
            bindings.toutiao.as_str(),
            bindings.toutiao_platform_screen_name.as_str(),
        ),
        (
            DouyinSessionBindingPlatform::Qq.label(),
            bindings.qq.as_str(),
            bindings.qq_platform_screen_name.as_str(),
        ),
        (
            DouyinSessionBindingPlatform::Google.label(),
            bindings.google.as_str(),
            bindings.google_platform_screen_name.as_str(),
        ),
        (
            DouyinSessionBindingPlatform::AppleId.label(),
            bindings.apple_id.as_str(),
            bindings.apple_id_platform_screen_name.as_str(),
        ),
        (
            DouyinSessionBindingPlatform::Wechat.label(),
            bindings.wechat.as_str(),
            bindings.wechat_platform_screen_name.as_str(),
        ),
    ] {
        if !value.is_empty() || !screen_name.is_empty() {
            labels.push(label);
        }
    }
    labels.join("｜")
}

fn normalize_douyin_binding_platform(value: &str) -> Option<DouyinSessionBindingPlatform> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.contains("qzone") || normalized.starts_with("qq") {
        Some(DouyinSessionBindingPlatform::Qq)
    } else if normalized.contains("google") {
        Some(DouyinSessionBindingPlatform::Google)
    } else if normalized.contains("weixin") || normalized.contains("wechat") {
        Some(DouyinSessionBindingPlatform::Wechat)
    } else if normalized.contains("apple") {
        Some(DouyinSessionBindingPlatform::AppleId)
    } else if normalized.contains("toutiao") || normalized.contains("news_article") {
        Some(DouyinSessionBindingPlatform::Toutiao)
    } else {
        None
    }
}

fn format_douyin_connect_binding_value(connect: &Value) -> String {
    let pairs: Vec<(&str, &str)> = vec![
        ("platform_uid", "platformUID"),
        ("open_id", "openId"),
        ("sec_platform_uid", "secPlatformUID"),
    ];
    pairs
        .into_iter()
        .filter_map(|(snake, camel)| {
            douyin_connect_field(connect, snake, camel).map(|value| format!("{snake}={value}"))
        })
        .collect::<Vec<_>>()
        .join("｜")
}

fn douyin_connect_screen_name(connect: &Value) -> Option<String> {
    first_non_empty_strings(&[
        douyin_connect_field(connect, "platform_screen_name", "platformScreenName"),
        douyin_connect_field(connect, "screen_name", "screenName"),
        douyin_connect_field(connect, "nickname", "nickName"),
        connect.get("name").and_then(douyin_normalize_json_value),
    ])
}

fn merge_douyin_binding_slot(
    value_slot: &mut String,
    screen_name_slot: &mut String,
    binding_value: String,
    screen_name: String,
) {
    append_unique_text(value_slot, binding_value);
    append_unique_text(screen_name_slot, screen_name);
}

fn append_unique_text(slot: &mut String, next: String) {
    if next.trim().is_empty() {
        return;
    }
    if slot.is_empty() {
        *slot = next;
        return;
    }
    if slot == &next || slot.contains(&next) {
        return;
    }
    slot.push('｜');
    slot.push_str(&next);
}

fn dedupe_owned_text(values: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for value in values {
        if value.trim().is_empty() || result.contains(&value) {
            continue;
        }
        result.push(value);
    }
    result
}

fn merge_douyin_local_account_identity(
    fallback: Option<&DouyinLocalAccountIdentity>,
    preferred: Option<&DouyinLocalAccountIdentity>,
) -> DouyinLocalAccountIdentity {
    let fallback = fallback.cloned().unwrap_or_default();
    let preferred = preferred.cloned().unwrap_or_default();

    DouyinLocalAccountIdentity {
        uid: first_non_empty_strings(&[Some(preferred.uid.clone()), Some(fallback.uid.clone())])
            .unwrap_or_default(),
        nickname: first_non_empty_strings(&[
            Some(preferred.nickname.clone()),
            Some(fallback.nickname.clone()),
        ])
        .unwrap_or_default(),
        sec_uid: first_non_empty_strings(&[
            Some(preferred.sec_uid.clone()),
            Some(fallback.sec_uid.clone()),
        ])
        .unwrap_or_default(),
        unique_id: first_non_empty_strings(&[
            Some(preferred.unique_id.clone()),
            Some(fallback.unique_id.clone()),
        ])
        .unwrap_or_default(),
        short_id: first_non_empty_strings(&[
            Some(preferred.short_id.clone()),
            Some(fallback.short_id.clone()),
        ])
        .unwrap_or_default(),
        phone_number: prefer_better_phone_number(
            Some(preferred.phone_number.clone()).filter(|value| !value.trim().is_empty()),
            Some(fallback.phone_number.clone()).filter(|value| !value.trim().is_empty()),
        )
        .unwrap_or_default(),
        register_time: first_non_empty_strings(&[
            Some(preferred.register_time.clone()),
            Some(fallback.register_time.clone()),
        ])
        .unwrap_or_default(),
        aweme_count: first_non_empty_strings(&[
            Some(preferred.aweme_count.clone()),
            Some(fallback.aweme_count.clone()),
        ])
        .unwrap_or_default(),
        following_count: first_non_empty_strings(&[
            Some(preferred.following_count.clone()),
            Some(fallback.following_count.clone()),
        ])
        .unwrap_or_default(),
        liked_count: first_non_empty_strings(&[
            Some(preferred.liked_count.clone()),
            Some(fallback.liked_count.clone()),
        ])
        .unwrap_or_default(),
        bindings: merge_douyin_session_bindings(fallback.bindings, preferred.bindings),
        has_password: preferred.has_password.or(fallback.has_password),
        is_verified: preferred.is_verified.or(fallback.is_verified),
        normal_functions: dedupe_owned_text(
            preferred
                .normal_functions
                .into_iter()
                .chain(fallback.normal_functions)
                .collect(),
        ),
    }
}

fn extract_query_param(query: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    query
        .split('&')
        .find_map(|part| part.strip_prefix(&prefix))
        .and_then(|value| urlencoding::decode(value).ok())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty())
}

fn parse_douyin_password_status_payload(payload: &Value) -> ParsedDouyinPasswordStatus {
    let data = payload.get("data").unwrap_or(payload);
    let user = data.get("user");
    let passport_account = payload
        .get("passportAccount")
        .or_else(|| data.get("passportAccount"));
    let app_user_info = passport_account
        .and_then(|value| value.get("appUserInfo"))
        .or_else(|| payload.get("appUserInfo"))
        .or_else(|| data.get("appUserInfo"));

    let has_password = [
        user.and_then(|u| u.get("has_password")),
        user.and_then(|u| u.get("hasPassword")),
        data.get("has_password"),
        data.get("hasPassword"),
        passport_account.and_then(|value| value.get("has_password")),
        passport_account.and_then(|value| value.get("hasPassword")),
        app_user_info.and_then(|value| value.get("has_password")),
        app_user_info.and_then(|value| value.get("hasPassword")),
    ]
    .into_iter()
    .flatten()
    .find_map(normalize_password_value);

    let register_time = first_json_string_from_paths(
        payload,
        &[
            &["data", "user", "user_create_time"],
            &["user", "user_create_time"],
            &["data", "user_create_time"],
            &["user_create_time"],
            &["passportAccount", "user_create_time"],
            &["appUserInfo", "user_create_time"],
            &["data", "user", "create_time"],
            &["user", "create_time"],
            &["data", "create_time"],
            &["create_time"],
        ],
    );

    ParsedDouyinPasswordStatus {
        has_password,
        screen_name: parse_douyin_screen_name(payload),
        register_time,
        bindings: parse_douyin_session_bindings(payload),
    }
}

fn merge_douyin_password_status(
    local: ParsedDouyinPasswordStatus,
    remote: Option<ParsedDouyinPasswordStatus>,
) -> ParsedDouyinPasswordStatus {
    let Some(remote) = remote else {
        return local;
    };

    ParsedDouyinPasswordStatus {
        has_password: remote.has_password.or(local.has_password),
        screen_name: remote.screen_name.or(local.screen_name),
        register_time: remote.register_time.or(local.register_time),
        bindings: merge_douyin_session_bindings(local.bindings, remote.bindings),
    }
}

fn has_douyin_password_status_data(status: &ParsedDouyinPasswordStatus) -> bool {
    status.has_password.is_some()
        || status.screen_name.is_some()
        || status.register_time.is_some()
        || !status.bindings.summary.is_empty()
}

fn parsed_douyin_password_status_to_result(
    source_zip: String,
    source_cookie_path: Option<String>,
    session_id: String,
    parsed: ParsedDouyinPasswordStatus,
    error: Option<String>,
) -> DouyinPasswordStatusResult {
    let status = match parsed.has_password {
        Some(true) => "ok".to_string(),
        Some(false) => "not_set".to_string(),
        None => "parse_error".to_string(),
    };
    let has_password = parsed.has_password;
    let account_name = parsed.screen_name;
    let register_time = parsed.register_time;
    let bindings = parsed.bindings;

    DouyinPasswordStatusResult {
        source_zip,
        source_cookie_path,
        session_id,
        has_password,
        account_name,
        register_time,
        bindings,
        status,
        error: match has_password {
            Some(_) => error,
            None => error.or(Some("douyin_has_password_not_found".to_string())),
        },
    }
}

fn merge_douyin_session_bindings(
    local: DouyinSessionBindings,
    remote: DouyinSessionBindings,
) -> DouyinSessionBindings {
    let mut merged = DouyinSessionBindings {
        summary: String::new(),
        toutiao: merge_prioritized_text(remote.toutiao, local.toutiao),
        toutiao_platform_screen_name: merge_prioritized_text(
            remote.toutiao_platform_screen_name,
            local.toutiao_platform_screen_name,
        ),
        qq: merge_prioritized_text(remote.qq, local.qq),
        qq_platform_screen_name: merge_prioritized_text(
            remote.qq_platform_screen_name,
            local.qq_platform_screen_name,
        ),
        google: merge_prioritized_text(remote.google, local.google),
        google_platform_screen_name: merge_prioritized_text(
            remote.google_platform_screen_name,
            local.google_platform_screen_name,
        ),
        apple_id: merge_prioritized_text(remote.apple_id, local.apple_id),
        apple_id_platform_screen_name: merge_prioritized_text(
            remote.apple_id_platform_screen_name,
            local.apple_id_platform_screen_name,
        ),
        wechat: merge_prioritized_text(remote.wechat, local.wechat),
        wechat_platform_screen_name: merge_prioritized_text(
            remote.wechat_platform_screen_name,
            local.wechat_platform_screen_name,
        ),
    };
    merged.summary = build_douyin_session_bindings_summary(&merged);
    merged
}

fn merge_prioritized_text(primary: String, fallback: String) -> String {
    if primary.trim().is_empty() {
        return fallback;
    }
    if fallback.trim().is_empty() || primary == fallback || primary.contains(&fallback) {
        return primary;
    }
    format!("{primary}｜{fallback}")
}

fn parse_douyin_certification_status_payload(payload: &Value) -> ParsedDouyinCertificationStatus {
    let data = payload.get("data").unwrap_or(payload);
    let passport_account = payload
        .get("passportAccount")
        .or_else(|| data.get("passportAccount"));
    let passport_raw_data = passport_account
        .and_then(|value| value.get("rawData"))
        .and_then(|value| value.get("data"))
        .or_else(|| payload.get("rawData").and_then(|value| value.get("data")))
        .or_else(|| data.get("rawData").and_then(|value| value.get("data")));
    let aweme_account = payload
        .get("awemeAccount")
        .or_else(|| data.get("awemeAccount"));
    let aweme_user = aweme_account
        .and_then(|value| value.get("rawData"))
        .and_then(|value| value.get("user"))
        .or_else(|| data.get("user"));
    let business_model = payload
        .get("businessModel")
        .or_else(|| data.get("businessModel"));

    let is_verified = [
        passport_raw_data.and_then(|value| value.get("user_verified")),
        payload.get("user_verified"),
        data.get("user_verified"),
        aweme_user.and_then(|value| value.get("user_verified")),
        aweme_user.and_then(|value| value.get("realname_verify_status")),
        aweme_user.and_then(|value| value.get("real_name_verify_status")),
        passport_raw_data.and_then(|value| value.get("realname_verify_status")),
        passport_raw_data.and_then(|value| value.get("real_name_verify_status")),
        business_model.and_then(|value| value.get("realNameVerifyStatus")),
        business_model.and_then(|value| value.get("realname_verify_status")),
    ]
    .into_iter()
    .flatten()
    .find_map(normalize_boolish_value);

    ParsedDouyinCertificationStatus {
        is_verified,
        screen_name: parse_douyin_screen_name(payload),
    }
}

fn parse_toutiao_certification_status_payload(payload: &Value) -> ParsedToutiaoCertificationStatus {
    let is_verified = payload
        .get("data")
        .and_then(|value| value.get("is_verified"))
        .or_else(|| payload.get("is_verified"))
        .and_then(normalize_boolish_value);

    ParsedToutiaoCertificationStatus { is_verified }
}

fn parse_toutiao_token_payload(payload: &Value) -> ParsedToutiaoTokenCheck {
    let top_message = payload
        .get("message")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.trim().is_empty());
    let profile = payload.get("profile").unwrap_or(&Value::Null);
    let profile_message = profile
        .get("message")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.trim().is_empty());
    let errno = profile.get("errno").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
            .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
    });
    let data = profile.get("data").unwrap_or(&Value::Null);
    let nickname = data
        .get("name")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.trim().is_empty());
    let uid = data
        .get("user_id")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.trim().is_empty() && value != "0");
    let register_time = data
        .get("create_time")
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.trim().is_empty());

    let top_success = top_message
        .as_deref()
        .is_some_and(|message| message.eq_ignore_ascii_case("success"));
    let profile_success = profile_message
        .as_deref()
        .is_some_and(|message| message.eq_ignore_ascii_case("success"));
    let has_explicit_failure = top_message
        .as_deref()
        .is_some_and(|message| !message.eq_ignore_ascii_case("success"))
        || profile_message
            .as_deref()
            .is_some_and(|message| !message.eq_ignore_ascii_case("success"))
        || errno.is_some_and(|value| value != 0);
    let is_logged_out = top_success && profile_success && errno == Some(0) && uid.is_none();
    let is_valid = if has_explicit_failure {
        Some(false)
    } else if top_success && profile_success && errno == Some(0) && uid.is_some() {
        Some(true)
    } else if is_logged_out {
        Some(false)
    } else {
        None
    };

    ParsedToutiaoTokenCheck {
        is_valid,
        message: if is_logged_out {
            Some("toutiao_token_not_logged_in".to_string())
        } else {
            profile_message.or(top_message)
        },
        nickname,
        uid,
        register_time,
    }
}

fn normalize_password_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => {
            let value = number
                .as_i64()
                .or_else(|| number.as_u64().map(|v| v as i64))?;
            Some(value != 0)
        }
        Value::String(text) => normalize_boolish_text(text),
        _ => None,
    }
}

fn normalize_boolish_value(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => {
            let value = number
                .as_i64()
                .or_else(|| number.as_u64().map(|v| v as i64))?;
            Some(value != 0)
        }
        Value::String(text) => normalize_boolish_text(text),
        _ => None,
    }
}

fn normalize_boolish_text(value: &str) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn extract_canonical_href(body: &str) -> Option<String> {
    let canonical_pos = body.find("rel=\"canonical\"")?;
    let rest = &body[canonical_pos..];
    let href_pos = rest.find("href=\"")?;
    let href_rest = &rest[href_pos + 6..];
    let end = href_rest.find('"')?;
    Some(href_rest[..end].to_string())
}

fn extract_og_url(body: &str) -> Option<String> {
    let marker = "property=\"og:url\"";
    let og_pos = body.find(marker)?;
    let rest = &body[og_pos..];
    let content_pos = rest.find("content=\"")?;
    let content_rest = &rest[content_pos + 9..];
    let end = content_rest.find('"')?;
    Some(content_rest[..end].to_string())
}

fn extract_between(value: &str, start: &str, end_char: char) -> Option<String> {
    let start_index = value.find(start)? + start.len();
    let tail = &value[start_index..];
    let end_index = tail.find(end_char)?;
    Some(tail[..end_index].to_string())
}

fn extract_token_tail(value: &str) -> Option<String> {
    let start_index = value.find("token/")? + 6;
    let tail = &value[start_index..];
    let trimmed = tail.split(['?', '#']).next().unwrap_or_default().trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_toutiao_secuid_from_url(value: &str) -> Option<String> {
    extract_between(value, "token/", '/').or_else(|| extract_token_tail(value))
}

fn emit_scan_progress(
    app: &AppHandle,
    stage: &str,
    message: &str,
    current: usize,
    total: usize,
    current_zip: Option<String>,
) {
    let safe_total = total.max(1);
    let safe_current = current.min(safe_total);
    let percent = ((safe_current as f64 / safe_total as f64) * 100.0).round();
    let _ = app.emit(
        "scan-progress",
        ScanProgressPayload {
            stage: stage.to_string(),
            message: message.to_string(),
            current: safe_current,
            total: safe_total,
            current_zip,
            percent,
        },
    );
}

fn binarycookies_flags_label(flags: u32) -> &'static str {
    match flags {
        0 => "",
        1 => "Secure",
        4 => "HttpOnly",
        5 => "Secure; HttpOnly",
        _ => "Unknown",
    }
}

fn apple_epoch_to_unix(value: f64) -> i64 {
    (value + 978_307_200.0).floor() as i64
}

fn apple_epoch_to_label(value: f64) -> String {
    apple_epoch_to_unix(value).to_string()
}

fn format_zip_lookup_error(inner_path: &str, error: ZipError) -> String {
    match error {
        ZipError::FileNotFound => format!("zip_entry_read_failed: file not found: {inner_path}"),
        other => format!("zip_entry_read_failed: {other}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_path,
            list_files,
            parse_file,
            export_file_result,
            export_app_result,
            move_zip_files,
            copy_zip_files,
            resolve_douyin_unique_id,
            resolve_toutiao_secuid,
            extract_douyin_request_params,
            check_douyin_password_status,
            check_douyin_certification_status,
            check_douyin_token_status,
            extract_douyin_account_credentials,
            check_toutiao_token_status,
            check_toutiao_certification_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn douyin_connect_field(connect: &Value, snake: &str, camel: &str) -> Option<String> {
    connect
        .get(snake)
        .or_else(|| connect.get(camel))
        .and_then(douyin_normalize_json_value)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Write;
    use std::time::Duration;
    use zip::write::SimpleFileOptions;

    fn write_test_zip(path: &Path, entries: &[&str]) {
        let file = File::create(path).expect("create test zip");
        let mut writer = zip::ZipWriter::new(file);
        for entry in entries {
            writer
                .start_file(*entry, SimpleFileOptions::default())
                .expect("start test entry");
            writer.write_all(b"test").expect("write test entry");
        }
        writer.finish().expect("finish test zip");
    }

    #[test]
    fn reuses_cached_app_file_path_index_without_rebuilding() {
        let temp_dir = tempdir().expect("tempdir");
        let cache_key = format!("test-app-index::{}", temp_dir.path().display());
        let builds = Cell::new(0);
        let first = get_or_build_app_file_path_index(&cache_key, || {
            builds.set(builds.get() + 1);
            Ok(vec!["batch/com.demo.app/Library/demo.plist".to_string()])
        })
        .expect("first index");
        let second = get_or_build_app_file_path_index(&cache_key, || {
            builds.set(builds.get() + 1);
            Ok(Vec::new())
        })
        .expect("cached index");

        assert_eq!(builds.get(), 1);
        assert_eq!(first, second);
    }

    #[test]
    fn app_file_path_cache_key_separates_apps() {
        assert_ne!(
            build_app_file_path_index_cache_key("sample.zip::123", "com.demo.one"),
            build_app_file_path_index_cache_key("sample.zip::123", "com.demo.two"),
        );
    }

    #[test]
    fn zip_cache_key_changes_after_file_modification() {
        let temp_dir = tempdir().expect("tempdir");
        let zip_path = temp_dir.path().join("sample.zip");
        fs::write(&zip_path, b"one").expect("initial file");
        let first = build_zip_cache_key(zip_path.to_string_lossy().as_ref()).expect("first key");
        std::thread::sleep(Duration::from_millis(5));
        fs::write(&zip_path, b"two-two").expect("modified file");
        let second = build_zip_cache_key(zip_path.to_string_lossy().as_ref()).expect("second key");
        assert_ne!(first, second);
    }

    #[test]
    fn finds_multiple_suffixes_from_one_zip_app_index() {
        let temp_dir = tempdir().expect("tempdir");
        let zip_path = temp_dir.path().join("apps.zip");
        write_test_zip(
            &zip_path,
            &[
                "batch/com.demo.app/Library/Preferences/demo.plist",
                "batch/com.demo.app/Library/Cookies/Cookies.binarycookies",
                "batch/com.other.app/Library/Preferences/other.plist",
            ],
        );
        let zip_text = zip_path.to_string_lossy();

        let plist = find_app_file_path(
            zip_text.as_ref(),
            "com.demo.app",
            &["Library/Preferences/demo.plist"],
        )
        .expect("plist lookup");
        let cookies = find_app_file_path(
            zip_text.as_ref(),
            "com.demo.app",
            &["Library/Cookies/Cookies.binarycookies"],
        )
        .expect("cookie lookup");

        assert_eq!(
            plist.as_deref(),
            Some("batch/com.demo.app/Library/Preferences/demo.plist")
        );
        assert_eq!(
            cookies.as_deref(),
            Some("batch/com.demo.app/Library/Cookies/Cookies.binarycookies")
        );
    }

    #[test]
    fn allows_only_known_package_target_directories() {
        assert!(is_allowed_zip_target_subdir("online"));
        assert!(is_allowed_zip_target_subdir("douyin_online"));
        assert!(is_allowed_zip_target_subdir("toutiao_online"));
        assert!(!is_allowed_zip_target_subdir("../outside"));
        assert!(!is_allowed_zip_target_subdir("custom"));
    }

    #[test]
    fn copies_zip_without_removing_source() {
        let temp_dir = tempdir().expect("tempdir");
        let source = temp_dir.path().join("shared.zip");
        fs::write(&source, b"zip-content").expect("source zip");

        let result = copy_zip_files_impl(
            vec![source.to_string_lossy().to_string()],
            "douyin_online".to_string(),
        )
        .expect("copy result");

        let destination = temp_dir.path().join("douyin_online/shared.zip");
        assert!(source.is_file());
        assert_eq!(fs::read(destination).expect("copied zip"), b"zip-content");
        assert!(result[0].contains("成功复制 1 个文件"));
    }

    #[test]
    fn copy_zip_does_not_overwrite_existing_destination() {
        let temp_dir = tempdir().expect("tempdir");
        let source = temp_dir.path().join("shared.zip");
        let destination_dir = temp_dir.path().join("toutiao_online");
        let destination = destination_dir.join("shared.zip");
        fs::write(&source, b"source").expect("source zip");
        fs::create_dir_all(&destination_dir).expect("destination dir");
        fs::write(&destination, b"existing").expect("existing zip");

        let error = copy_zip_files_impl(
            vec![source.to_string_lossy().to_string()],
            "toutiao_online".to_string(),
        )
        .expect_err("existing destination must fail");

        assert!(error.contains("目标文件已存在"));
        assert_eq!(fs::read(destination).expect("existing zip"), b"existing");
    }

    #[test]
    fn copy_zip_rejects_non_zip_source() {
        let temp_dir = tempdir().expect("tempdir");
        let source = temp_dir.path().join("notes.txt");
        fs::write(&source, b"not a zip").expect("text file");

        let error = copy_zip_files_impl(
            vec![source.to_string_lossy().to_string()],
            "douyin_online".to_string(),
        )
        .expect_err("non-zip source must fail");

        assert!(error.contains("仅支持移动或复制 ZIP 文件"));
    }

    #[test]
    fn parses_successful_toutiao_token_payload() {
        let payload = json!({
            "message": "success",
            "profile": {
                "errno": 0,
                "message": "success",
                "data": {
                    "name": "测试用户",
                    "user_id": 819616220453017_u64,
                    "create_time": "1778145951"
                }
            }
        });

        let parsed = parse_toutiao_token_payload(&payload);

        assert_eq!(parsed.is_valid, Some(true));
        assert_eq!(parsed.nickname.as_deref(), Some("测试用户"));
        assert_eq!(parsed.uid.as_deref(), Some("819616220453017"));
        assert_eq!(parsed.register_time.as_deref(), Some("1778145951"));
    }

    #[test]
    fn treats_explicit_toutiao_token_business_failure_as_invalid() {
        let payload = json!({
            "message": "error",
            "profile": { "errno": 1034, "message": "user not login" }
        });

        let parsed = parse_toutiao_token_payload(&payload);

        assert_eq!(parsed.is_valid, Some(false));
        assert_eq!(parsed.message.as_deref(), Some("user not login"));
    }

    #[test]
    fn treats_successful_toutiao_token_payload_without_uid_as_logged_out() {
        let payload = json!({
            "message": "success",
            "profile": {
                "errno": 0,
                "message": "success",
                "data": { "name": "缺少 UID" }
            }
        });

        let parsed = parse_toutiao_token_payload(&payload);

        assert_eq!(parsed.is_valid, Some(false));
        assert_eq!(
            parsed.message.as_deref(),
            Some("toutiao_token_not_logged_in")
        );
    }

    #[test]
    fn selects_toutiao_token_and_device_id_with_documented_fallbacks() {
        let source = json!({
            "kTTAccountTokenGuardXTTToken": "guard-token",
            "bdaccount_session_x_tt_token": "session-token",
            "FlowSaveDeviceId": { "deviceId": "primary-device" },
            "kOldDeviceIDStorageKey": "old-device"
        });

        assert_eq!(toutiao_token_value(&source), "guard-token");
        assert_eq!(toutiao_device_id(&source), "primary-device");

        let fallback = json!({
            "bdaccount_session_x_tt_token": "session-token",
            "kOldDeviceIDStorageKey": "old-device"
        });
        assert_eq!(toutiao_token_value(&fallback), "session-token");
        assert_eq!(toutiao_device_id(&fallback), "old-device");
    }

    #[test]
    fn selects_newest_relevant_toutiao_token_cookie() {
        let parsed_cookies = json!({
            "cookieHeader": "odin_tt=header-fallback",
            "cookies": [
                {"name":"odin_tt","value":"relevant-old","domain":".toutiaoapi.com","created":100.0},
                {"name":"odin_tt","value":"unrelated-new","domain":".example.com","created":999.0},
                {"name":"odin_tt","value":"relevant-new","domain":".snssdk.com","created":200.0}
            ]
        });

        assert_eq!(
            toutiao_cookie_value(&parsed_cookies, "odin_tt").as_deref(),
            Some("relevant-new")
        );
    }

    #[test]
    #[ignore = "requires TOUTIAO_TOKEN_TEST_ZIP and live network"]
    fn checks_toutiao_token_live_fixture() {
        let zip_path = std::env::var("TOUTIAO_TOKEN_TEST_ZIP").expect("fixture path");

        let result = check_toutiao_token_status_impl(zip_path).expect("token check");

        println!(
            "status={} uid={} nickname={} register_time={}",
            result.status,
            result.uid.as_deref().unwrap_or("-"),
            result.nickname.as_deref().unwrap_or("-"),
            result.register_time.as_deref().unwrap_or("-")
        );
        assert!(!result.device_id.is_empty());
        assert!(!result.iid.is_empty());
        assert!(!result.token_preview.contains("--"));
    }

    #[test]
    fn parses_urlencoded_douyin_multi_sessions() {
        let encoded = "3981761322158107%3Acc194d2ff6dfe86111cb43232f31ad32%7C7657159319138911281%3A62c82321d9edb605126e53d032d6c1a5";

        let sessions = parse_douyin_multi_session_map(encoded);

        assert_eq!(
            sessions.get("3981761322158107").map(String::as_str),
            Some("cc194d2ff6dfe86111cb43232f31ad32")
        );
        assert_eq!(
            sessions.get("7657159319138911281").map(String::as_str),
            Some("62c82321d9edb605126e53d032d6c1a5")
        );
    }

    #[test]
    fn extracts_douyin_token_cluster_entries_from_plist_bytes() {
        let bytes = br#"prefix {"7657159319138911281":{"expireTime":1296000,"userID":"4548803299210744891","authTime":1782822908.520302,"secUID":"MS4wLjABAAAAqjmowbWbtqZp63LNLJoCCY7j0XlLtU2fssxiSbY1OZdGd4ZAYEi7-FnhqJds6t2S","isSlient":true,"state":0,"openID":"_000bCEzVsPRrRvr-VKjYsy_sRyHVBboV-S8","accessToken":"act.3.Qnhl5h--d6-kQ95bnaTrmd8j_D-DdyZUA8r2sX9uGzbIAej4kP0aBdn-KMxDkmtcylG8qeD1ZYHGmFoE8OXa2HqDwomYZ742ZgSbgGIKHqrvz9kbQ3Diua4CzsCrqLd8HL9Qj0fVkjaft04XBuVc2ImizJy_r5FArDvA6sge771mqA4mUeUfyysGQEg=_hl"}} suffix"#;

        let cluster = extract_douyin_token_cluster_map(bytes);
        let entry = cluster
            .get("7657159319138911281")
            .expect("token cluster entry");

        assert!(entry.access_token.starts_with("act.3."));
        assert_eq!(entry.open_id, "_000bCEzVsPRrRvr-VKjYsy_sRyHVBboV-S8");
        assert_eq!(
            entry.sec_uid,
            "MS4wLjABAAAAqjmowbWbtqZp63LNLJoCCY7j0XlLtU2fssxiSbY1OZdGd4ZAYEi7-FnhqJds6t2S"
        );
        assert_eq!(entry.auth_time_label, "1782822908.520302057");
    }

    #[test]
    fn reads_cookie_value_from_joined_header() {
        let cookie_header = "sessionid=abc123; odin_tt=odin_value; passport_csrf_token=token";

        assert_eq!(
            extract_cookie_value(cookie_header, "sessionid").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            extract_cookie_value(cookie_header, "odin_tt").as_deref(),
            Some("odin_value")
        );
    }

    #[test]
    fn reads_douyin_session_id_from_fallback_cookie_keys() {
        let cookie_header = "sessionid_ss=shadow123; sid_tt=shadow456; odin_tt=odin_value";

        assert_eq!(
            extract_douyin_session_id(cookie_header).as_deref(),
            Some("shadow123")
        );

        let cookie_header = "sid_tt=shadow456; odin_tt=odin_value";
        assert_eq!(
            extract_douyin_session_id(cookie_header).as_deref(),
            Some("shadow456")
        );
    }

    #[test]
    fn parses_douyin_password_status_from_data_field() {
        let payload = json!({
            "message": "success",
            "data": {
                "has_password": 1,
                "screen_name": "demo_user",
                "user_create_time": 1719936000
            }
        });

        let status = parse_douyin_password_status_payload(&payload);

        assert_eq!(status.has_password, Some(true));
        assert_eq!(status.screen_name.as_deref(), Some("demo_user"));
        assert_eq!(status.register_time.as_deref(), Some("1719936000"));
    }

    #[test]
    fn parses_douyin_password_status_from_nested_user_field() {
        let payload = json!({
            "data": {
                "user": {
                    "has_password": false,
                    "name": "nested_user",
                    "user_create_time": 1719936123
                }
            }
        });

        let status = parse_douyin_password_status_payload(&payload);

        assert_eq!(status.has_password, Some(false));
        assert_eq!(status.screen_name.as_deref(), Some("nested_user"));
        assert_eq!(status.register_time.as_deref(), Some("1719936123"));
    }

    #[test]
    fn parses_douyin_password_status_from_local_passport_account() {
        let payload = json!({
            "passportAccount": {
                "hasPassword": true,
                "screenName": "local_user"
            }
        });

        let status = parse_douyin_password_status_payload(&payload);

        assert_eq!(status.has_password, Some(true));
        assert_eq!(status.screen_name.as_deref(), Some("local_user"));
    }

    #[test]
    fn parses_douyin_local_account_fallback_fields_for_batch_table() {
        let payload = json!({
            "passportAccount": {
                "hasPassword": true,
                "phoneNumber": "+86 13800000000"
            },
            "awemeAccount": {
                "userID": "123456",
                "nickname": "local_user",
                "rawData": {
                    "user": {
                        "uid": "123456",
                        "sec_uid": "MS4wLjABAAAA-local-demo",
                        "unique_id": "douyin_demo",
                        "short_id": "8023",
                        "create_time": 1719936000,
                        "aweme_count": 12,
                        "following_count": 34,
                        "total_favorited": 567,
                        "realname_verify_status": 2
                    }
                }
            },
            "connects": [
                {
                    "platform": "qzone_sns",
                    "platform_screen_name": "黑白配",
                    "open_id": "8E023E6D34CC7F21DD44F2314A968DD7"
                }
            ]
        });

        let profile = parse_douyin_local_account_identity_from_payload(&payload)
            .expect("local account profile");

        assert_eq!(profile.phone_number, "+86 13800000000");
        assert_eq!(profile.register_time, "1719936000");
        assert_eq!(profile.aweme_count, "12");
        assert_eq!(profile.following_count, "34");
        assert_eq!(profile.liked_count, "567");
        assert_eq!(profile.bindings.summary, "QQ");
        assert_eq!(profile.bindings.qq_platform_screen_name, "黑白配");
        assert_eq!(profile.has_password, Some(true));
        assert_eq!(profile.is_verified, Some(true));
        assert_eq!(
            profile.normal_functions,
            vec!["改密功能".to_string(), "实名正常".to_string()]
        );
    }

    #[test]
    fn does_not_use_connect_platform_screen_name_as_account_name() {
        let payload = json!({
            "message": "success",
            "data": {
                "has_password": 1,
                "connects": [
                    {
                        "platform": "google",
                        "platform_screen_name": "gdmb ztvq",
                        "platform_uid": "114710961078614248609"
                    },
                    {
                        "platform": "qzone_sns",
                        "platform_screen_name": "黑白配",
                        "open_id": "8E023E6D34CC7F21DD44F2314A968DD7"
                    }
                ]
            }
        });

        let status = parse_douyin_password_status_payload(&payload);

        assert_eq!(status.has_password, Some(true));
        assert_eq!(status.screen_name, None);
        assert_eq!(status.bindings.google_platform_screen_name, "gdmb ztvq");
        assert_eq!(status.bindings.qq_platform_screen_name, "黑白配");
    }

    #[test]
    fn parses_douyin_session_bindings_from_connects() {
        let payload = json!({
            "message": "success",
            "data": {
                "connects": [
                    {
                        "platform": "qzone_sns",
                        "platform_screen_name": "黑白配",
                        "platform_uid": "UID_987A132FC1FA6A148DF5DABD13AD518D",
                        "open_id": "8E023E6D34CC7F21DD44F2314A968DD7"
                    },
                    {
                        "platform": "google",
                        "platform_screen_name": "gdmb ztvq",
                        "platform_uid": "114710961078614248609",
                        "sec_platform_uid": "MS4wLjABAAAAGGooqo3hpJgDaiXCbjjqvypXls367msWCNKtOINM2vATqKiu9qOF1VOsKqnjhZ9h"
                    }
                ]
            }
        });

        let bindings = parse_douyin_session_bindings(&payload);

        assert_eq!(bindings.summary, "QQ｜谷歌");
        assert_eq!(
            bindings.qq,
            "platform_uid=UID_987A132FC1FA6A148DF5DABD13AD518D｜open_id=8E023E6D34CC7F21DD44F2314A968DD7"
        );
        assert_eq!(bindings.qq_platform_screen_name, "黑白配");
        assert_eq!(
            bindings.google,
            "platform_uid=114710961078614248609｜sec_platform_uid=MS4wLjABAAAAGGooqo3hpJgDaiXCbjjqvypXls367msWCNKtOINM2vATqKiu9qOF1VOsKqnjhZ9h"
        );
        assert_eq!(bindings.google_platform_screen_name, "gdmb ztvq");
    }

    #[test]
    fn groups_sdk_user_info_connects_with_toutiao_v2_snake_case_user_id() {
        let root = json!({
            "connects": [
                {
                    "platform": "toutiao_v2",
                    "user_id": "1001",
                    "platform_screen_name": "用户944125371366",
                    "platform_uid": "392956133781594",
                    "sec_platform_uid": "MS4wLjABAAAATy9QXyauFIrIhShlhPNv8Lbbj3GBWiV0ZD2Y7EK39sU",
                    "open_id": "7658031779017950218"
                }
            ]
        });

        let grouped = group_sdk_user_info_connects_by_uid(&root);
        let connects = grouped.get("1001").expect("grouped connects for uid 1001");
        let payload = json!({ "connects": connects });
        let bindings = parse_douyin_session_bindings_for_uid(&payload, Some("1001"));

        assert_eq!(bindings.summary, "头条");
        assert_eq!(
            bindings.toutiao,
            "platform_uid=392956133781594｜open_id=7658031779017950218｜sec_platform_uid=MS4wLjABAAAATy9QXyauFIrIhShlhPNv8Lbbj3GBWiV0ZD2Y7EK39sU"
        );
        assert_eq!(bindings.toutiao_platform_screen_name, "用户944125371366");
    }

    #[test]
    fn filters_douyin_connects_by_matching_user_id() {
        let payload = json!({
            "awemeAccount": {
                "userID": "1001"
            },
            "connects": [
                {
                    "platform": "qzone_sns",
                    "user_id": "1001",
                    "platform_screen_name": "qq_user_1001",
                    "open_id": "OPEN_ID_1001"
                },
                {
                    "platform": "google",
                    "user_id": "2002",
                    "platform_screen_name": "google_user_2002",
                    "platform_uid": "GOOGLE_UID_2002"
                }
            ]
        });

        let status = parse_douyin_password_status_payload(&payload);

        assert_eq!(status.screen_name, None);
        assert_eq!(status.bindings.summary, "QQ");
        assert_eq!(status.bindings.qq_platform_screen_name, "qq_user_1001");
        assert_eq!(status.bindings.google_platform_screen_name, "");
    }

    #[test]
    fn prefers_native_name_over_binding_platform_screen_name() {
        let payload = json!({
            "data": {
                "user": {
                    "name": "抖音主账号名"
                },
                "connects": [
                    {
                        "platform": "google",
                        "platform_screen_name": "google_binding_name",
                        "platform_uid": "114710961078614248609"
                    }
                ]
            }
        });

        let status = parse_douyin_password_status_payload(&payload);

        assert_eq!(status.screen_name.as_deref(), Some("抖音主账号名"));
        assert_eq!(
            status.bindings.google_platform_screen_name,
            "google_binding_name"
        );
    }

    #[test]
    fn propagates_root_connects_to_matching_local_accounts() {
        let payload = json!({
            "connects": [
                {
                    "platform": "qzone_sns",
                    "user_id": "1001",
                    "platform_screen_name": "qq_user_1001",
                    "open_id": "OPEN_ID_1001"
                },
                {
                    "platform": "google",
                    "user_id": "2002",
                    "platform_screen_name": "google_user_2002",
                    "platform_uid": "GOOGLE_UID_2002"
                }
            ],
            "NS.objects": [
                {
                    "awemeAccount": {
                        "userID": "1001",
                        "nickname": "user_1001",
                        "rawData": {
                            "user": {
                                "uid": "1001"
                            }
                        }
                    }
                },
                {
                    "awemeAccount": {
                        "userID": "2002",
                        "nickname": "user_2002",
                        "rawData": {
                            "user": {
                                "uid": "2002"
                            }
                        }
                    }
                }
            ]
        });

        let accounts = parse_douyin_local_account_identities(&payload);
        let first = accounts
            .iter()
            .find(|item| item.uid == "1001")
            .expect("uid 1001");
        let second = accounts
            .iter()
            .find(|item| item.uid == "2002")
            .expect("uid 2002");

        assert_eq!(first.bindings.summary, "QQ");
        assert_eq!(first.bindings.qq_platform_screen_name, "qq_user_1001");
        assert_eq!(second.bindings.summary, "谷歌");
        assert_eq!(
            second.bindings.google_platform_screen_name,
            "google_user_2002"
        );
    }

    #[test]
    fn extracts_login_data_connects_from_raw_bytes_without_lossy_utf8_matching() {
        let user_id_1 = "7147109610786142486";
        let user_id_2 = "8147109610786142486";
        let qq_uid_hex = "0123456789abcdef0123456789ABCDEF";
        let google_uid_prefixed = "114710961078614248609";
        let google_uid_vgoogle = "224710961078614248610";

        let mut dat = Vec::new();
        dat.extend_from_slice(&[0x9a, 0x09, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]);
        dat.extend_from_slice(b"bplist00");
        dat.extend_from_slice(b"prefix user ");
        dat.extend_from_slice(user_id_1.as_bytes());
        dat.extend_from_slice(b" block ");
        dat.extend_from_slice(b" google name gdmb ztvq ");
        dat.extend_from_slice(google_uid_vgoogle.as_bytes());
        dat.extend_from_slice(b"Vgoogle");
        dat.extend(std::iter::repeat_n(0x00, 24));
        dat.extend_from_slice(b"UID_");
        dat.extend_from_slice(qq_uid_hex.as_bytes());
        dat.extend(std::iter::repeat_n(0xff, 96));
        dat.extend_from_slice(b"qzone_sns");
        dat.extend(std::iter::repeat_n(0x00, 48));
        dat.extend_from_slice(b"user ");
        dat.extend_from_slice(user_id_2.as_bytes());
        dat.extend_from_slice(b" connect ");
        dat.extend_from_slice(&[0x5f, 0x10, 0x15]);
        dat.extend_from_slice(google_uid_prefixed.as_bytes());
        dat.extend(std::iter::repeat_n(0xff, 64));
        dat.extend_from_slice(b"google");
        dat.extend(std::iter::repeat_n(0x00, 64));

        let connects = extract_login_data_connects(&dat);

        assert!(connects.iter().any(|(uid, connect)| {
            uid == user_id_1
                && connect.get("platform").and_then(Value::as_str) == Some("qzone_sns")
                && connect.get("platformUID").and_then(Value::as_str)
                    == Some("UID_0123456789abcdef0123456789ABCDEF")
        }));
        assert!(connects.iter().any(|(uid, connect)| {
            uid == user_id_1
                && connect.get("platform").and_then(Value::as_str) == Some("google")
                && connect.get("platformUID").and_then(Value::as_str) == Some(google_uid_vgoogle)
        }));
        assert!(connects.iter().any(|(uid, connect)| {
            uid == user_id_2
                && connect.get("platform").and_then(Value::as_str) == Some("google")
                && connect.get("platformUID").and_then(Value::as_str) == Some(google_uid_prefixed)
        }));
    }

    #[test]
    fn cleans_login_data_screen_name_artifacts() {
        assert_eq!(clean_login_data_screen_name("Yuepx juvx"), "uepx juvx");
        assert_eq!(clean_login_data_screen_name("Yrlwl nytd"), "rlwl nytd");
        assert_eq!(clean_login_data_screen_name("jkEqYycrf hbth"), "ycrf hbth");
        assert_eq!(clean_login_data_screen_name("fbds kupk"), "fbds kupk");
    }

    #[test]
    fn ignores_datetime_like_numeric_ids_when_matching_login_data_connects() {
        let real_uid = "7657219377347249209";
        let fake_uid = "202607010046585";
        let qq_uid_hex = "0123456789abcdef0123456789ABCDEF";

        let mut dat = Vec::new();
        dat.extend_from_slice(b"user ");
        dat.extend_from_slice(real_uid.as_bytes());
        dat.extend_from_slice(
            std::iter::repeat_n(b'X', 512)
                .collect::<Vec<_>>()
                .as_slice(),
        );
        dat.extend_from_slice(b"log_id_");
        dat.extend_from_slice(fake_uid.as_bytes());
        dat.extend_from_slice(b"A7877CB93F9998DA8F2 marker ");
        dat.extend_from_slice(b"UID_");
        dat.extend_from_slice(qq_uid_hex.as_bytes());
        dat.extend(std::iter::repeat_n(0xff, 64));
        dat.extend_from_slice(b"qzone_sns");

        let connects = extract_login_data_connects(&dat);

        assert!(connects.iter().any(|(uid, connect)| {
            uid == real_uid
                && connect.get("platform").and_then(Value::as_str) == Some("qzone_sns")
                && connect.get("platformUID").and_then(Value::as_str)
                    == Some("UID_0123456789abcdef0123456789ABCDEF")
        }));
        assert!(!connects.iter().any(|(uid, _)| uid == fake_uid));
    }

    #[test]
    fn extracts_unique_id_near_sec_uid_from_mmkv_bytes() {
        let sec_uid =
            "MS4wLjABAAAATyAmV90qOJ9_PFX1LE8ou0OqyS5vCYdqKm2QyWl5V-dl06Qy3dcGfObuGlvVWVoi";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"prefix");
        bytes.extend_from_slice(&[0x01, 0x02, 0x0b]);
        bytes.extend_from_slice(b"87244195544");
        bytes.push(0x02);
        bytes.extend_from_slice(sec_uid.as_bytes());
        bytes.extend_from_slice(b"suffix");

        assert_eq!(
            extract_unique_id_near_sec_uid(&bytes, sec_uid).as_deref(),
            Some("87244195544")
        );
    }

    #[test]
    fn extracts_uid_sec_uid_pairs_from_accountsaaskit_bytes() {
        let uid = "7657219377347249209";
        let sec_uid =
            "MS4wLjABAAAATyAmV90qOJ9_PFX1LE8ou0OqyS5vCYdqKm2QyWl5V-dl06Qy3dcGfObuGlvVWVoi";
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"prefix");
        bytes.extend_from_slice(uid.as_bytes());
        bytes.extend_from_slice(&[b'_', 0x10, b'L']);
        bytes.extend_from_slice(sec_uid.as_bytes());
        bytes.push(0x01);
        bytes.extend_from_slice(b"suffix");

        let pairs = extract_uid_sec_uid_pairs_from_accountsaaskit(&bytes);

        assert_eq!(pairs.get(uid).map(String::as_str), Some(sec_uid));
    }

    #[test]
    fn keeps_douyin_login_data_dat_as_analyzable_app_file() {
        assert!(should_analyze_app_file(
            "com.ss.iphone.ugc.Aweme",
            "Library/loginData.dat",
            "misc",
            "unknown"
        ));
    }

    #[test]
    fn keeps_douyin_sdk_archiver_as_analyzable_app_file() {
        assert!(should_analyze_app_file(
            "com.ss.iphone.ugc.Aweme",
            "Documents/ttaccountSDKUserInfo.archiver",
            "misc",
            "archiver"
        ));
    }

    #[test]
    fn parses_qq_screen_name_from_connect_field_aliases() {
        let payload = json!({
            "connects": [
                {
                    "platform": "qzone_sns",
                    "platform_uid": "UID_987A132FC1FA6A148DF5DABD13AD518D",
                    "screenName": "黑白配"
                }
            ]
        });

        let bindings = parse_douyin_session_bindings(&payload);

        assert_eq!(bindings.qq_platform_screen_name, "黑白配");
    }

    #[test]
    fn merges_remote_bindings_even_when_local_has_password() {
        let local = ParsedDouyinPasswordStatus {
            has_password: Some(true),
            screen_name: Some("local_user".to_string()),
            register_time: None,
            bindings: DouyinSessionBindings::default(),
        };
        let remote = ParsedDouyinPasswordStatus {
            has_password: Some(true),
            screen_name: Some("remote_user".to_string()),
            register_time: Some("1719936000".to_string()),
            bindings: DouyinSessionBindings {
                summary: "QQ｜谷歌".to_string(),
                qq: "platform_uid=qq_uid".to_string(),
                qq_platform_screen_name: "黑白配".to_string(),
                google: "platform_uid=google_uid".to_string(),
                google_platform_screen_name: "gdmb ztvq".to_string(),
                ..DouyinSessionBindings::default()
            },
        };

        let merged = merge_douyin_password_status(local, Some(remote));

        assert_eq!(merged.has_password, Some(true));
        assert_eq!(merged.screen_name.as_deref(), Some("remote_user"));
        assert_eq!(merged.register_time.as_deref(), Some("1719936000"));
        assert_eq!(merged.bindings.summary, "QQ｜谷歌");
        assert_eq!(merged.bindings.qq_platform_screen_name, "黑白配");
        assert_eq!(merged.bindings.google_platform_screen_name, "gdmb ztvq");
    }

    #[test]
    fn formats_douyin_request_error_without_full_url_noise() {
        let message = format_douyin_request_error_message(
            DouyinTokenEndpoint::SafetyPortrait,
            "error sending request for url (https://api5-normal-c-hl.amemv.com/aweme/v3/user/safety/portrait/?foo=bar): operation timed out",
        );

        assert_eq!(
            message,
            "safety_portrait_request_failed: operation timed out"
        );
    }

    #[test]
    fn formats_douyin_request_error_without_reason_suffix() {
        let message = format_douyin_request_error_message(
            DouyinTokenEndpoint::SafetyPortrait,
            "error sending request for url (https://api5-normal-c-hl.amemv.com/aweme/v3/user/safety/portrait/?foo=bar)",
        );

        assert_eq!(
            message,
            "safety_portrait_request_failed: request_send_failed"
        );
    }

    #[test]
    fn parses_toutiao_certification_status_from_nested_data() {
        let payload = json!({
            "data": {
                "is_verified": true
            }
        });

        let status = parse_toutiao_certification_status_payload(&payload);

        assert_eq!(status.is_verified, Some(true));
    }

    #[test]
    fn builds_douyin_safety_portrait_query_for_token_check() {
        let source = json!({
            "extra_info": r#"{
                "device_id":"3848738162626205",
                "iid":"2370994534881229",
                "app_version":"23.2.0",
                "version_code":"23.2.0",
                "build_number":"232018",
                "os_version":"15.6.1",
                "os_api":"18",
                "device_type":"iPhone10,3",
                "channel":"App Store",
                "cdid":"220AAB8F-EDE2-46FF-BB14-A80179E829AB",
                "mcc_mnc":"46000",
                "screen_width":"1125",
                "appTheme":"light",
                "js_sdk_version":"02760003",
                "tma_jssdk_version":"02760003",
                "aid":"1128",
                "minor_status":"0",
                "device_platform":"iphone",
                "app_name":"aweme",
                "package":"com.ss.iphone.ugc.Aweme"
            }"#,
            "kTTInstallAppVersion": "23.2.0",
            "gurd_kit_update_version_code": "232018",
            "kBDUGPushSDKAID": "1128"
        });

        let query =
            douyin_build_token_check_query(&source, "", DouyinTokenEndpoint::SafetyPortrait);

        assert_eq!(
            extract_query_param(&query, "package").as_deref(),
            Some("com.ss.iphone.ugc.Aweme")
        );
        assert_eq!(
            extract_query_param(&query, "device_id").as_deref(),
            Some("3848738162626205")
        );
        assert_eq!(
            extract_query_param(&query, "iid").as_deref(),
            Some("2370994534881229")
        );
        assert_eq!(
            extract_query_param(&query, "slide_guide_has_shown").as_deref(),
            Some("1")
        );
        assert_eq!(
            extract_query_param(&query, "app_version").as_deref(),
            Some("23.2.0")
        );
        assert_eq!(
            extract_query_param(&query, "build_number").as_deref(),
            Some("232018")
        );
    }

    #[test]
    fn parses_douyin_profile_self_payload_as_valid_token() {
        let payload = json!({
            "status_code": 0,
            "user": {
                "uid": "123456",
                "sec_uid": "MS4wLjABAAAA-demo",
                "nickname": "demo_user",
                "mobile": "138****0000",
                "create_time": 1719936000,
                "aweme_count": 12,
                "following_count": 34,
                "total_favorited": 567
            }
        });

        let parsed = parse_douyin_token_check_payload(&payload);

        assert_eq!(parsed.is_valid, Some(true));
        assert_eq!(parsed.uid.as_deref(), Some("123456"));
        assert_eq!(parsed.sec_uid.as_deref(), Some("MS4wLjABAAAA-demo"));
        assert_eq!(parsed.nickname.as_deref(), Some("demo_user"));
        assert_eq!(parsed.phone_number.as_deref(), Some("138****0000"));
        assert_eq!(parsed.register_time.as_deref(), Some("1719936000"));
        assert_eq!(parsed.aweme_count.as_deref(), Some("12"));
        assert_eq!(parsed.following_count.as_deref(), Some("34"));
        assert_eq!(parsed.liked_count.as_deref(), Some("567"));
    }

    #[test]
    fn parses_douyin_user_create_time_as_register_time() {
        let payload = json!({
            "status_code": 0,
            "data": {
                "user": {
                    "uid": "123456",
                    "user_create_time": 1719936123
                }
            }
        });

        let parsed = parse_douyin_token_check_payload(&payload);

        assert_eq!(parsed.register_time.as_deref(), Some("1719936123"));
    }

    #[test]
    fn parses_douyin_mobile_change_phone_number() {
        let payload = json!({
            "new_mobile_info": {
                "lastNewMobile": "",
                "newMobile": "+1 5103705260",
                "newPhone": "5103705260"
            }
        });

        let phone_number = parse_douyin_mobile_change_payload(&payload);

        assert_eq!(phone_number.as_deref(), Some("+1 5103705260"));
    }

    #[test]
    fn falls_back_to_new_mobile_when_new_phone_missing() {
        let payload = json!({
            "new_mobile_info": {
                "newMobile": "+86 13800000000"
            }
        });

        let phone_number = parse_douyin_mobile_change_payload(&payload);

        assert_eq!(phone_number.as_deref(), Some("+86 13800000000"));
    }

    #[test]
    fn prefers_passport_account_phone_number_with_country_code() {
        let payload = json!({
            "passportAccount": {
                "phoneNumber": "+852 51234567"
            },
            "new_mobile_info": {
                "newPhone": "51234567",
                "newMobile": "+852 51234567"
            }
        });

        let phone_number = parse_douyin_mobile_change_payload(&payload);

        assert_eq!(phone_number.as_deref(), Some("+852 51234567"));
    }

    #[test]
    fn prefers_unmasked_new_mobile_over_masked_account_phone() {
        let payload = json!({
            "passportAccount": {
                "phoneNumber": "+1******60"
            },
            "new_mobile_info": {
                "newMobile": "+1 9292269324",
                "newPhone": "9292269324"
            }
        });

        let phone_number = parse_douyin_mobile_change_payload(&payload);

        assert_eq!(phone_number.as_deref(), Some("+1 9292269324"));
    }

    #[test]
    fn builds_phone_number_from_country_code_and_new_phone() {
        let payload = json!({
            "passportAccount": {
                "rawData": {
                    "data": {
                        "country_code": 86
                    }
                }
            },
            "new_mobile_info": {
                "newPhone": "13800000000"
            }
        });

        let phone_number = parse_douyin_mobile_change_payload(&payload);

        assert_eq!(phone_number.as_deref(), Some("+86 13800000000"));
    }

    #[test]
    fn parses_douyin_certification_status_from_local_payload() {
        let payload = json!({
            "passportAccount": {
                "rawData": {
                    "data": {
                        "user_verified": false
                    }
                },
                "screenName": "local_user"
            },
            "awemeAccount": {
                "rawData": {
                    "user": {
                        "realname_verify_status": 1
                    }
                }
            }
        });

        let status = parse_douyin_certification_status_payload(&payload);

        assert_eq!(status.is_verified, Some(false));
        assert_eq!(status.screen_name.as_deref(), Some("local_user"));
    }

    #[test]
    fn parses_douyin_function_items_from_func_elements() {
        let payload = json!({
            "data": {
                "func_elements": [
                    {
                        "func_name": "登录功能",
                        "func_avaliable": true
                    },
                    {
                        "func_name": "投稿功能",
                        "func_avaliable": false
                    },
                    {
                        "func_name": "评论功能",
                        "func_avaliable": false
                    }
                ]
            }
        });

        let items = parse_douyin_function_items(&payload);

        assert_eq!(
            items,
            vec![
                DouyinFunctionItem {
                    func_name: "登录功能".to_string(),
                    func_available: true,
                },
                DouyinFunctionItem {
                    func_name: "投稿功能".to_string(),
                    func_available: false,
                },
                DouyinFunctionItem {
                    func_name: "评论功能".to_string(),
                    func_available: false,
                },
            ]
        );
    }

    #[test]
    fn parses_douyin_profile_other_identity_fields() {
        let payload = json!({
            "user": {
                "uid": "1645304352875224",
                "sec_uid": "MS4wLjABAAAA-demo",
                "unique_id": "64873343492"
            },
            "status_code": 0
        });

        let parsed = parse_douyin_profile_other_payload(&payload, "MS4wLjABAAAA-fallback");

        assert_eq!(parsed.uid.as_deref(), Some("1645304352875224"));
        assert_eq!(parsed.sec_uid.as_deref(), Some("MS4wLjABAAAA-demo"));
        assert_eq!(parsed.unique_id.as_deref(), Some("64873343492"));
    }

    #[test]
    fn resolves_multiple_zip_paths_from_multiline_input() {
        let temp_dir = tempdir().expect("tempdir");
        let zip_a = temp_dir.path().join("a.zip");
        let zip_b = temp_dir.path().join("b.zip");
        fs::write(&zip_a, b"").expect("zip_a");
        fs::write(&zip_b, b"").expect("zip_b");

        let input = format!("{}\n{}", zip_a.to_string_lossy(), zip_b.to_string_lossy());

        let scan_input = resolve_scan_input(&input).expect("scan_input");

        assert_eq!(scan_input.source_mode, "files");
        assert_eq!(
            scan_input.zip_paths,
            vec![
                zip_a.to_string_lossy().to_string(),
                zip_b.to_string_lossy().to_string()
            ]
        );
    }

    #[test]
    fn builds_virtual_path_for_backup_entries() {
        let inner_path = build_backup_virtual_path(
            "com.ss.iphone.ugc.Aweme",
            "Library/Preferences/com.ss.iphone.ugc.Aweme.plist",
        );

        assert_eq!(
            inner_path,
            "__manifest_backup__/com.ss.iphone.ugc.Aweme/Library/Preferences/com.ss.iphone.ugc.Aweme.plist"
        );
        assert_eq!(
            split_entry_path(&inner_path),
            Some((
                Some("__manifest_backup__".to_string()),
                "com.ss.iphone.ugc.Aweme".to_string(),
                "Library/Preferences/com.ss.iphone.ugc.Aweme.plist".to_string()
            ))
        );
    }

    #[test]
    fn parses_backup_manifest_domain_into_app_id() {
        assert_eq!(
            backup_domain_app_id("AppDomain-com.ss.iphone.ugc.Aweme"),
            Some("com.ss.iphone.ugc.Aweme")
        );
        assert_eq!(
            backup_domain_app_id("AppDomainPlugin-com.ss.iphone.ugc.Aweme"),
            None
        );
    }

    #[test]
    fn builds_chunk_ranges_without_out_of_bounds_for_uneven_splits() {
        let ranges = build_chunk_ranges(91, 20);

        assert_eq!(ranges.first(), Some(&(0, 5)));
        assert_eq!(ranges.last(), Some(&(90, 91)));
        assert_eq!(ranges.len(), 19);

        let covered = ranges
            .iter()
            .flat_map(|(start, end)| *start..*end)
            .collect::<Vec<_>>();

        assert_eq!(covered.len(), 91);
        assert_eq!(covered.first(), Some(&0));
        assert_eq!(covered.last(), Some(&90));
        assert_eq!(covered, (0..91).collect::<Vec<_>>());
    }

    #[test]
    fn resolves_nested_backup_directories_from_directory_input() {
        let temp_dir = tempdir().expect("tempdir");
        let backup_root = temp_dir
            .path()
            .join("24组")
            .join("2026年06月22日_20时36分29秒_iPhone")
            .join("00008030-000931E222D1402E");
        fs::create_dir_all(&backup_root).expect("create backup root");
        fs::write(backup_root.join("Manifest.db"), b"manifest").expect("manifest");

        let scan_input =
            resolve_scan_input(temp_dir.path().to_string_lossy().as_ref()).expect("scan_input");

        assert_eq!(scan_input.source_mode, "directory");
        assert_eq!(
            scan_input.zip_paths,
            vec![backup_root.to_string_lossy().to_string()]
        );
    }

    #[test]
    fn finds_app_file_path_in_backup_directory_manifest() {
        let temp_dir = tempdir().expect("tempdir");
        let backup_root = temp_dir.path().join("00008030-000931E222D1402E");
        fs::create_dir_all(&backup_root).expect("create backup root");
        let manifest_path = backup_root.join("Manifest.db");
        let connection = Connection::open(&manifest_path).expect("open manifest");
        connection
            .execute(
                "CREATE TABLE Files (
                    fileID TEXT,
                    domain TEXT,
                    relativePath TEXT,
                    flags INTEGER
                )",
                [],
            )
            .expect("create files table");
        connection
            .execute(
                "INSERT INTO Files (fileID, domain, relativePath, flags)
                 VALUES (?1, ?2, ?3, 1)",
                (
                    "abc1234567890def",
                    "AppDomain-com.ss.iphone.ugc.Aweme",
                    "Library/Preferences/com.ss.iphone.ugc.Aweme.plist",
                ),
            )
            .expect("insert row");
        drop(connection);

        let found = find_app_file_path(
            backup_root.to_string_lossy().as_ref(),
            "com.ss.iphone.ugc.Aweme",
            &["Library/Preferences/com.ss.iphone.ugc.Aweme.plist"],
        )
        .expect("find path");

        assert_eq!(
            found,
            Some(build_backup_virtual_path(
                "com.ss.iphone.ugc.Aweme",
                "Library/Preferences/com.ss.iphone.ugc.Aweme.plist"
            ))
        );
    }

    #[test]
    fn reads_backup_virtual_entry_bytes_from_directory_source() {
        let temp_dir = tempdir().expect("tempdir");
        let backup_root = temp_dir.path().join("00008030-000931E222D1402E");
        let file_id = "ab1234567890def1234567890abcdef12345678";
        let actual_parent = backup_root.join(&file_id[..2]);
        fs::create_dir_all(&actual_parent).expect("create hashed dir");
        fs::write(actual_parent.join(file_id), br#"{"demo":true}"#).expect("write payload");

        let manifest_path = backup_root.join("Manifest.db");
        let connection = Connection::open(&manifest_path).expect("open manifest");
        connection
            .execute(
                "CREATE TABLE Files (
                    fileID TEXT,
                    domain TEXT,
                    relativePath TEXT,
                    flags INTEGER
                )",
                [],
            )
            .expect("create files table");
        connection
            .execute(
                "INSERT INTO Files (fileID, domain, relativePath, flags)
                 VALUES (?1, ?2, ?3, 1)",
                (
                    file_id,
                    "AppDomain-com.ss.iphone.ugc.Aweme",
                    "Library/Preferences/com.ss.iphone.ugc.Aweme.plist",
                ),
            )
            .expect("insert row");
        drop(connection);

        let bytes = read_zip_entry_bytes(
            backup_root.to_string_lossy().as_ref(),
            &build_backup_virtual_path(
                "com.ss.iphone.ugc.Aweme",
                "Library/Preferences/com.ss.iphone.ugc.Aweme.plist",
            ),
        )
        .expect("read bytes");

        assert_eq!(bytes, br#"{"demo":true}"#);
    }
}

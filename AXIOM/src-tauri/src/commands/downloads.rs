use futures_util::StreamExt;
use parking_lot::RwLock;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;

use super::tabs::CommandResult;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadInfo {
    pub id: String,
    pub url: String,
    pub file_path: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub state: String,
    pub progress: f64,
    pub risk_level: String,
    pub needs_warning: bool,
    pub can_resume: bool,
    pub hash: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl From<axiom_core::Download> for DownloadInfo {
    fn from(download: axiom_core::Download) -> Self {
        let risk_level = match download.risk_level() {
            axiom_core::RiskLevel::Safe => "safe",
            axiom_core::RiskLevel::Warning => "warning",
            axiom_core::RiskLevel::Dangerous => "dangerous",
        }
        .to_string();

        let state = download.state.as_str().to_string();
        let progress = download.progress();
        let needs_warning = download.needs_warning();
        let can_resume = download.can_resume();
        let created_at = download.created_at.to_rfc3339();
        let completed_at = download.completed_at.map(|dt| dt.to_rfc3339());

        Self {
            id: download.id,
            url: download.url,
            file_path: download.file_path,
            file_name: download.file_name,
            mime_type: download.mime_type,
            total_bytes: download.total_bytes,
            downloaded_bytes: download.downloaded_bytes,
            state,
            progress,
            risk_level,
            needs_warning,
            can_resume,
            hash: download.hash,
            created_at,
            completed_at,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DownloadControl {
    Continue,
    Pause,
    Cancel,
}

#[derive(Clone)]
pub struct DownloadRuntime {
    jobs: Arc<RwLock<HashMap<String, tokio::sync::watch::Sender<DownloadControl>>>>,
}

impl Default for DownloadRuntime {
    fn default() -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

fn emit_download_update(app: &AppHandle, download: axiom_core::Download) {
    let _ = app.emit("download-updated", DownloadInfo::from(download));
}

fn best_effort_file_name(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(name) = parsed
            .path_segments()
            .and_then(|mut s| s.next_back())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return name.to_string();
        }
    }
    "download".to_string()
}

#[tauri::command]
pub fn list_downloads(state: State<'_, AppState>) -> CommandResult<Vec<DownloadInfo>> {
    match state.with_browser(|browser| Ok(browser.download_manager().list_downloads())) {
        Ok(mut downloads) => {
            downloads.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            CommandResult::ok(downloads.into_iter().map(DownloadInfo::from).collect())
        }
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn create_download(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    file_name: Option<String>,
) -> CommandResult<DownloadInfo> {
    let file_name = file_name.unwrap_or_else(|| best_effort_file_name(&url));

    match state.with_browser(|browser| browser.create_download(url, file_name)) {
        Ok(download) => {
            emit_download_update(&app, download.clone());
            CommandResult::ok(download.into())
        }
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadInfo> {
    let runtime = app.state::<DownloadRuntime>().inner().clone();
    if runtime.jobs.read().contains_key(&download_id) {
        let existing = state.with_browser(|browser| {
            browser
                .download_manager()
                .get_download(&download_id)
                .map_err(Into::into)
        });
        return match existing {
            Ok(download) => CommandResult::ok(download.into()),
            Err(e) => CommandResult::err(e.to_string()),
        };
    }

    let manager = match state.with_browser(|browser| Ok(browser.download_manager().clone())) {
        Ok(m) => m,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let download = match manager.start_download(&download_id) {
        Ok(d) => d,
        Err(e) => return CommandResult::err(e.to_string()),
    };
    emit_download_update(&app, download.clone());

    let (tx, rx) = tokio::sync::watch::channel(DownloadControl::Continue);
    runtime.jobs.write().insert(download_id.clone(), tx);

    let jobs = runtime.jobs.clone();
    let app_for_task = app.clone();
    let manager_for_task = manager.clone();
    tokio::spawn(async move {
        run_download_task(app_for_task, manager_for_task, download_id.clone(), rx).await;
        jobs.write().remove(&download_id);
    });

    CommandResult::ok(download.into())
}

#[tauri::command]
pub fn resume_download(
    app: AppHandle,
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadInfo> {
    let runtime = app.state::<DownloadRuntime>().inner().clone();
    if runtime.jobs.read().contains_key(&download_id) {
        return CommandResult::err("Download already running".to_string());
    }

    let manager = match state.with_browser(|browser| Ok(browser.download_manager().clone())) {
        Ok(m) => m,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let download = match manager.resume_download(&download_id) {
        Ok(d) => d,
        Err(e) => return CommandResult::err(e.to_string()),
    };
    emit_download_update(&app, download.clone());

    let (tx, rx) = tokio::sync::watch::channel(DownloadControl::Continue);
    runtime.jobs.write().insert(download_id.clone(), tx);

    let jobs = runtime.jobs.clone();
    let app_for_task = app.clone();
    let manager_for_task = manager.clone();
    tokio::spawn(async move {
        run_download_task(app_for_task, manager_for_task, download_id.clone(), rx).await;
        jobs.write().remove(&download_id);
    });

    CommandResult::ok(download.into())
}

#[tauri::command]
pub fn pause_download(
    app: AppHandle,
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadInfo> {
    let runtime = app.state::<DownloadRuntime>().inner().clone();
    if let Some(tx) = runtime.jobs.read().get(&download_id).cloned() {
        let _ = tx.send(DownloadControl::Pause);
        match state.with_browser(|browser| {
            browser
                .download_manager()
                .get_download(&download_id)
                .map_err(Into::into)
        }) {
            Ok(download) => return CommandResult::ok(download.into()),
            Err(e) => return CommandResult::err(e.to_string()),
        }
    }

    let manager = match state.with_browser(|browser| Ok(browser.download_manager().clone())) {
        Ok(m) => m,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match manager.pause_download(&download_id) {
        Ok(download) => {
            emit_download_update(&app, download.clone());
            CommandResult::ok(download.into())
        }
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn cancel_download(
    app: AppHandle,
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<DownloadInfo> {
    let runtime = app.state::<DownloadRuntime>().inner().clone();
    if let Some(tx) = runtime.jobs.read().get(&download_id).cloned() {
        let _ = tx.send(DownloadControl::Cancel);
        match state.with_browser(|browser| {
            browser
                .download_manager()
                .get_download(&download_id)
                .map_err(Into::into)
        }) {
            Ok(download) => return CommandResult::ok(download.into()),
            Err(e) => return CommandResult::err(e.to_string()),
        }
    }

    let manager = match state.with_browser(|browser| Ok(browser.download_manager().clone())) {
        Ok(m) => m,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match manager.cancel_download(&download_id) {
        Ok(download) => {
            emit_download_update(&app, download.clone());
            CommandResult::ok(download.into())
        }
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn reveal_download(
    app: AppHandle,
    state: State<'_, AppState>,
    download_id: String,
) -> CommandResult<()> {
    let download = match state.with_browser(|browser| {
        browser
            .download_manager()
            .get_download(&download_id)
            .map_err(Into::into)
    }) {
        Ok(d) => d,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match app
        .opener()
        .reveal_item_in_dir(Path::new(&download.file_path))
    {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

async fn run_download_task(
    app: AppHandle,
    manager: axiom_core::DownloadManager,
    download_id: String,
    mut control: tokio::sync::watch::Receiver<DownloadControl>,
) {
    let mut download = match manager.get_download(&download_id) {
        Ok(d) => d,
        Err(e) => {
            let _ = manager.fail_download(&download_id, &e.to_string());
            return;
        }
    };

    let url = download.url.clone();
    let path = PathBuf::from(download.file_path.clone());
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let client = reqwest::Client::new();
    let mut offset = download.downloaded_bytes;

    let request = || {
        let mut req = client.get(url.clone());
        if offset > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={offset}-"));
        }
        req
    };

    let response = match request().send().await {
        Ok(r) => r,
        Err(e) => {
            if let Ok(d) = manager.fail_download(&download_id, &e.to_string()) {
                emit_download_update(&app, d);
            }
            return;
        }
    };

    if !response.status().is_success() {
        if let Ok(d) = manager.fail_download(&download_id, &format!("HTTP {}", response.status())) {
            emit_download_update(&app, d);
        }
        return;
    }

    if offset > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        offset = 0;
    }

    let content_length = response.content_length();
    let total = content_length.map(|len| len.saturating_add(offset));

    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let _ = manager.set_mime_type(&download_id, mime_type);

    let mut opts = tokio::fs::OpenOptions::new();
    opts.create(true).write(true);
    if offset > 0 {
        opts.append(true);
    } else {
        opts.truncate(true);
        download.downloaded_bytes = 0;
        let _ = manager.update_progress(&download_id, 0, total);
    }

    let mut file = match opts.open(&path).await {
        Ok(f) => f,
        Err(e) => {
            if let Ok(d) = manager.fail_download(&download_id, &e.to_string()) {
                emit_download_update(&app, d);
            }
            return;
        }
    };

    let mut downloaded = offset;
    let mut stream = response.bytes_stream();
    let mut last_persist = Instant::now();

    loop {
        tokio::select! {
            _ = control.changed() => {
                let action = *control.borrow();
                match action {
                    DownloadControl::Pause => {
                        let _ = file.flush().await;
                        if let Ok(d) = manager.pause_download(&download_id) {
                            emit_download_update(&app, d);
                        }
                        return;
                    }
                    DownloadControl::Cancel => {
                        let _ = file.flush().await;
                        if let Ok(d) = manager.cancel_download(&download_id) {
                            emit_download_update(&app, d);
                        }
                        return;
                    }
                    DownloadControl::Continue => {}
                }
            }
            chunk = stream.next() => {
                let chunk = match chunk {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(e)) => {
                        if let Ok(d) = manager.fail_download(&download_id, &e.to_string()) {
                            emit_download_update(&app, d);
                        }
                        return;
                    }
                    None => break,
                };

                if file.write_all(&chunk).await.is_err() {
                    if let Ok(d) = manager.fail_download(&download_id, "Failed to write file") {
                        emit_download_update(&app, d);
                    }
                    return;
                }

                downloaded = downloaded.saturating_add(chunk.len() as u64);

                if last_persist.elapsed() >= Duration::from_millis(250) {
                    last_persist = Instant::now();
                    if let Ok(d) = manager.update_progress(&download_id, downloaded, total) {
                        emit_download_update(&app, d);
                    }
                }
            }
        }
    }

    let _ = file.flush().await;
    let _ = manager.update_progress(&download_id, downloaded, total);

    match compute_sha256_hex(path.clone()).await {
        Ok(hash) => {
            if let Ok(d) = manager.complete_download(&download_id, Some(hash)) {
                emit_download_update(&app, d);
            }
        }
        Err(_) => {
            if let Ok(d) = manager.complete_download(&download_id, None) {
                emit_download_update(&app, d);
            }
        }
    }
}

async fn compute_sha256_hex(path: PathBuf) -> std::io::Result<String> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(path)?;
        let mut reader = std::io::BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8192];

        loop {
            let n = std::io::Read::read(&mut reader, &mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }

        let digest = hasher.finalize();
        let mut out = String::with_capacity(digest.len() * 2);
        for b in digest {
            out.push_str(&format!("{:02x}", b));
        }
        Ok(out)
    })
    .await
    .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())))
}

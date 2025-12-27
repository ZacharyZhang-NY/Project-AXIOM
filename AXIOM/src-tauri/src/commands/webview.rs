//! WebView management commands
//!
//! Handles creating and managing child webviews for tab content.
//! Each tab gets its own child webview within the main window.

use parking_lot::RwLock;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::webview::{DownloadEvent, NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, Window};

use super::tabs::CommandResult;
use crate::commands::downloads::DownloadInfo;
use crate::state::AppState;

const PRIVACY_INIT_SCRIPT: &str = r#"
(() => {
  try {
    // Disable WebRTC (PRD 5.4: WebRTC disabled globally)
    if ('RTCPeerConnection' in window) {
      try { window.RTCPeerConnection = undefined; } catch {}
      try { delete window.RTCPeerConnection; } catch {}
    }
    if ('webkitRTCPeerConnection' in window) {
      try { window.webkitRTCPeerConnection = undefined; } catch {}
      try { delete window.webkitRTCPeerConnection; } catch {}
    }

    // Disable Notification prompts by default (PRD 5.4: Notifications deny by default)
    if ('Notification' in window && typeof Notification === 'function') {
      try { Notification.requestPermission = () => Promise.resolve('denied'); } catch {}
    }
  } catch {}
})();
"#;

#[derive(Clone, Serialize)]
struct NewWindowRequestPayload {
    url: String,
    source_tab_id: String,
}

/// Manages webviews for tabs
pub struct WebviewManager {
    /// Map of window_label::tab_id -> webview label
    webviews: Arc<RwLock<HashMap<String, String>>>,
    /// Current bounds for content area (per window label)
    bounds: Arc<RwLock<HashMap<String, ContentBounds>>>,
}

#[derive(Clone, Copy)]
pub struct ContentBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for ContentBounds {
    fn default() -> Self {
        Self {
            x: 260.0, // sidebar width
            y: 48.0,  // toolbar height
            width: 1020.0,
            height: 752.0,
        }
    }
}

impl WebviewManager {
    pub fn new() -> Self {
        Self {
            webviews: Arc::new(RwLock::new(HashMap::new())),
            bounds: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn key(window_label: &str, tab_id: &str) -> String {
        format!("{}::{}", window_label, tab_id)
    }

    pub fn get_webview_label(&self, window_label: &str, tab_id: &str) -> Option<String> {
        self.webviews
            .read()
            .get(&Self::key(window_label, tab_id))
            .cloned()
    }

    pub fn register_webview(&self, window_label: &str, tab_id: String, label: String) {
        self.webviews
            .write()
            .insert(Self::key(window_label, &tab_id), label);
    }

    pub fn unregister_webview(&self, window_label: &str, tab_id: &str) -> Option<String> {
        self.webviews
            .write()
            .remove(&Self::key(window_label, tab_id))
    }

    pub fn get_all_labels(&self, window_label: &str) -> Vec<String> {
        let prefix = format!("{}::", window_label);
        self.webviews
            .read()
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(_, value)| value.clone())
            .collect()
    }

    pub fn get_bounds(&self, window_label: &str) -> ContentBounds {
        self.bounds
            .read()
            .get(window_label)
            .copied()
            .unwrap_or_default()
    }

    pub fn set_bounds(&self, window_label: &str, bounds: ContentBounds) {
        self.bounds.write().insert(window_label.to_string(), bounds);
    }
}

impl Default for WebviewManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for WebviewManager {
    fn clone(&self) -> Self {
        Self {
            webviews: Arc::clone(&self.webviews),
            bounds: Arc::clone(&self.bounds),
        }
    }
}

#[tauri::command]
pub async fn create_webview(
    app: AppHandle,
    window: Window,
    tab_id: String,
    url: String,
) -> CommandResult<String> {
    let window_label = window.label().to_string();
    let webview_label = format!("content-{}-{}", window_label.as_str(), tab_id.as_str());

    tracing::info!(
        window_label = %window_label,
        tab_id = %tab_id,
        url = %url,
        "Create webview requested"
    );

    // Ensure we don't create duplicates
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    if let Some(existing_label) = manager.get_webview_label(&window_label, &tab_id) {
        if app.get_webview(&existing_label).is_some() {
            return CommandResult::ok(existing_label);
        }
        manager.unregister_webview(&window_label, &tab_id);
    }

    // Get content bounds from manager
    let bounds = manager.get_bounds(&window_label);

    // Create the webview URL
    let webview_url = if url == "about:blank" || url.is_empty() {
        match "about:blank".parse::<url::Url>() {
            Ok(parsed) => WebviewUrl::External(parsed),
            Err(_) => return CommandResult::err("Invalid about:blank URL".to_string()),
        }
    } else {
        match url.parse::<url::Url>() {
            Ok(parsed) => WebviewUrl::External(parsed),
            Err(_) => return CommandResult::err(format!("Invalid URL: {}", url)),
        }
    };

    let ui_label = super::ui_webview_label(&window_label);

    let app_handle_for_load = app.clone();
    let tab_id_for_load = tab_id.clone();
    let app_handle_for_title = app.clone();
    let tab_id_for_title = tab_id.clone();
    let app_handle_for_download = app.clone();
    let ui_label_for_load = ui_label.clone();
    let ui_label_for_title = ui_label.clone();
    let app_handle_for_navigation = app.clone();
    let ui_label_for_navigation = ui_label.clone();
    let app_handle_for_new_window = app.clone();
    let ui_label_for_new_window = ui_label.clone();
    let tab_id_for_new_window = tab_id.clone();

    // Build the child webview
    let mut webview_builder = WebviewBuilder::new(&webview_label, webview_url)
        .transparent(false)
        .auto_resize()
        .enable_clipboard_access()
        .initialization_script_for_all_frames(PRIVACY_INIT_SCRIPT);

    if let Some(data_directory) = webview_data_directory(&app, &url) {
        webview_builder = webview_builder.data_directory(data_directory);
    }

    let webview_builder = webview_builder
        .on_navigation(move |url| {
            if matches!(url.scheme(), "tauri" | "about" | "axiom") {
                return true;
            }

            let url_str = url.as_str().to_string();
            if let Some(state) = app_handle_for_navigation.try_state::<AppState>() {
                if let Ok(should_block) =
                    state.with_browser(|browser| Ok(browser.should_block_url(&url_str)))
                {
                    if should_block {
                        let _ = app_handle_for_navigation.emit_to(
                            ui_label_for_navigation.as_str(),
                            "navigation-blocked",
                            url_str,
                        );
                        return false;
                    }
                }
            }

            true
        })
        .on_page_load(move |webview, payload| {
            let url = payload.url().to_string();
            if let Some(state) = app_handle_for_load.try_state::<AppState>() {
                match payload.event() {
                    PageLoadEvent::Started => {
                        let _ = state.with_browser(|browser| {
                            browser.update_tab_url_if_changed(&tab_id_for_load, &url)
                        });
                    }
                    PageLoadEvent::Finished => {
                        let Ok((autofill_enabled, name, email, password_save_enabled)) = state
                            .with_browser(|browser| {
                                Ok((
                                    browser.get_autofill_enabled()?,
                                    browser.get_autofill_name()?,
                                    browser.get_autofill_email()?,
                                    browser.get_password_save_prompt_enabled()?,
                                ))
                            })
                        else {
                            return;
                        };

                        let Some(parsed) = url::Url::parse(&url).ok() else {
                            return;
                        };
                        if parsed.scheme() != "http" && parsed.scheme() != "https" {
                            return;
                        }

                        if !password_save_enabled {
                            let _ = webview.eval(
                                r#"(() => {
  try {
    for (const el of document.querySelectorAll('input[type="password"]')) {
      try { el.setAttribute('autocomplete', 'off'); } catch {}
    }
    for (const form of document.querySelectorAll('form')) {
      try {
        if (form.querySelector('input[type="password"]')) {
          form.setAttribute('autocomplete', 'off');
        }
      } catch {}
    }
  } catch {}
})();"#,
                            );
                        }

                        let name = name.unwrap_or_default();
                        let email = email.unwrap_or_default();
                        if !autofill_enabled || (name.trim().is_empty() && email.trim().is_empty())
                        {
                            return;
                        }

                        let name_json =
                            serde_json::to_string(&name).unwrap_or_else(|_| "\"\"".to_string());
                        let email_json =
                            serde_json::to_string(&email).unwrap_or_else(|_| "\"\"".to_string());

                        let script = format!(
                            r#"(() => {{
  try {{
    const profileName = {name_json};
    const profileEmail = {email_json};
    const setIfEmpty = (el, value) => {{
      if (!el) return false;
      if (typeof value !== 'string' || !value.trim()) return false;
      if (typeof el.value === 'string' && el.value.trim()) return false;
      el.value = value;
      el.dispatchEvent(new Event('input', {{ bubbles: true }}));
      el.dispatchEvent(new Event('change', {{ bubbles: true }}));
      return true;
    }};

    if (profileEmail && profileEmail.trim()) {{
      const emailSelectors = [
        'input[type="email"]',
        'input[autocomplete="email"]',
        'input[name*="email" i]',
        'input[id*="email" i]',
      ];
      for (const el of document.querySelectorAll(emailSelectors.join(','))) {{
        setIfEmpty(el, profileEmail);
      }}
    }}

    if (profileName && profileName.trim()) {{
      const nameSelectors = [
        'input[autocomplete="name"]',
        'input[name="name" i]',
        'input[name*="full" i][name*="name" i]',
        'input[id*="name" i]',
      ];
      for (const el of document.querySelectorAll(nameSelectors.join(','))) {{
        if (String(el.type || '').toLowerCase() === 'password') continue;
        setIfEmpty(el, profileName);
      }}
    }}
  }} catch {{}}
}})();"#,
                            name_json = name_json,
                            email_json = email_json
                        );

                        let _ = webview.eval(&script);
                    }
                }
            }

            let _ = app_handle_for_load.emit_to(ui_label_for_load.as_str(), "tabs-updated", ());
        })
        .on_document_title_changed(move |_webview, title| {
            if let Some(state) = app_handle_for_title.try_state::<AppState>() {
                let _ = state.with_browser(|browser| {
                    browser.set_tab_title(&tab_id_for_title, title.clone())
                });
            }

            let _ = app_handle_for_title.emit_to(ui_label_for_title.as_str(), "tabs-updated", ());
        })
        .on_new_window(move |url, _features| {
            let _ = app_handle_for_new_window.emit_to(
                ui_label_for_new_window.as_str(),
                "new-window-requested",
                NewWindowRequestPayload {
                    url: url.as_str().to_string(),
                    source_tab_id: tab_id_for_new_window.clone(),
                },
            );
            NewWindowResponse::Deny
        })
        .on_download(move |_webview, event| {
            if let DownloadEvent::Requested { url, destination } = event {
                let file_name = destination
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("download")
                    .to_string();

                if let Some(state) = app_handle_for_download.try_state::<AppState>() {
                    if let Ok(download) = state
                        .with_browser(|browser| browser.create_download(url.to_string(), file_name))
                    {
                        let _ = app_handle_for_download
                            .emit("download-updated", DownloadInfo::from(download));
                    }
                }

                return false;
            }

            true
        });

    // Add as child of the invoking window
    match window.add_child(
        webview_builder,
        LogicalPosition::new(bounds.x, bounds.y),
        LogicalSize::new(bounds.width, bounds.height),
    ) {
        Ok(webview) => {
            // Start hidden
            let _ = webview.hide();

            // Register in manager
            manager.register_webview(&window_label, tab_id.clone(), webview_label.clone());

            tracing::info!(label = %webview_label, tab_id = %tab_id, "Created child webview");
            CommandResult::ok(webview_label)
        }
        Err(e) => {
            tracing::error!(
                label = %webview_label,
                tab_id = %tab_id,
                error = %e,
                "Failed to create child webview"
            );
            CommandResult::err(format!("Failed to create webview: {}", e))
        }
    }
}

fn webview_data_directory(app: &AppHandle, url: &str) -> Option<std::path::PathBuf> {
    let base = app.path().app_data_dir().ok()?;
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_lowercase()))
        .unwrap_or_else(|| "blank".to_string());

    let mut safe = String::with_capacity(host.len());
    for ch in host.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }

    Some(base.join("webview-partitions").join(safe))
}

#[tauri::command]
pub async fn navigate_webview(
    app: AppHandle,
    window: Window,
    tab_id: String,
    url: String,
) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let window_label = window.label();

    let label = match manager.get_webview_label(window_label, &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    let parsed_url: url::Url = match url.parse() {
        Ok(u) => u,
        Err(_) => return CommandResult::err(format!("Invalid URL: {}", url)),
    };

    match webview.navigate(parsed_url) {
        Ok(_) => {
            tracing::info!(label = %label, url = %url, "Navigated webview");
            CommandResult::ok(())
        }
        Err(e) => CommandResult::err(format!("Navigation failed: {}", e)),
    }
}

#[tauri::command]
pub async fn show_webview(app: AppHandle, window: Window, tab_id: String) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    // Hide all other content webviews (but not the UI webview)
    let window_label = window.label();
    let all_labels = manager.get_all_labels(window_label);
    for label in &all_labels {
        if let Some(webview) = app.get_webview(label) {
            let _ = webview.hide();
        }
    }

    // Show the requested webview
    let label = match manager.get_webview_label(window_label, &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    match webview.show() {
        Ok(_) => {
            tracing::info!(label = %label, "Showing webview");
            CommandResult::ok(())
        }
        Err(e) => CommandResult::err(format!("Failed to show webview: {}", e)),
    }
}

#[tauri::command]
pub async fn close_webview(app: AppHandle, window: Window, tab_id: String) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let label = match manager.unregister_webview(window.label(), &tab_id) {
        Some(l) => l,
        None => return CommandResult::ok(()), // Already closed
    };

    if let Some(webview) = app.get_webview(&label) {
        let _ = webview.close();
    }

    tracing::info!(label = %label, "Closed webview");
    CommandResult::ok(())
}

#[tauri::command]
pub async fn set_webview_bounds(
    app: AppHandle,
    window: Window,
    tab_id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    // Update stored bounds
    let window_label = window.label();
    manager.set_bounds(
        window_label,
        ContentBounds {
            x,
            y,
            width,
            height,
        },
    );

    let label = match manager.get_webview_label(window_label, &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    // Position is relative to the parent window
    let position = LogicalPosition::new(x, y);
    let size = LogicalSize::new(width, height);

    if let Err(e) = webview.set_position(position) {
        return CommandResult::err(format!("Failed to set position: {}", e));
    }

    if let Err(e) = webview.set_size(size) {
        return CommandResult::err(format!("Failed to set size: {}", e));
    }

    CommandResult::ok(())
}

/// Update all webview positions when window resizes
#[tauri::command]
pub async fn update_all_webview_bounds(
    app: AppHandle,
    window: Window,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    // Update stored bounds
    let window_label = window.label();
    manager.set_bounds(
        window_label,
        ContentBounds {
            x,
            y,
            width,
            height,
        },
    );

    // Position is relative to the parent window
    let position = LogicalPosition::new(x, y);
    let size = LogicalSize::new(width, height);

    // Update all content webviews
    let all_labels = manager.get_all_labels(window_label);
    for label in all_labels {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.set_position(position);
            let _ = webview.set_size(size);
        }
    }

    CommandResult::ok(())
}

#[tauri::command]
pub async fn reload_webview(app: AppHandle, window: Window, tab_id: String) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let label = match manager.get_webview_label(window.label(), &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    match webview.reload() {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(format!("Reload failed: {}", e)),
    }
}

#[tauri::command]
pub async fn force_reload_webview(
    app: AppHandle,
    window: Window,
    tab_id: String,
) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let label = match manager.get_webview_label(window.label(), &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    if webview.eval("location.reload(true)").is_ok() {
        return CommandResult::ok(());
    }

    match webview.reload() {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(format!("Force reload failed: {}", e)),
    }
}

#[tauri::command]
pub async fn stop_webview_loading(
    app: AppHandle,
    window: Window,
    tab_id: String,
) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let label = match manager.get_webview_label(window.label(), &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    match webview.eval("window.stop()") {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(format!("Stop loading failed: {}", e)),
    }
}

#[tauri::command]
pub async fn webview_back(app: AppHandle, window: Window, tab_id: String) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let label = match manager.get_webview_label(window.label(), &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    match webview.eval("history.back()") {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(format!("Back navigation failed: {}", e)),
    }
}

#[tauri::command]
pub async fn webview_forward(app: AppHandle, window: Window, tab_id: String) -> CommandResult<()> {
    let manager = match app.try_state::<WebviewManager>() {
        Some(m) => m,
        None => return CommandResult::err("WebviewManager not found".to_string()),
    };

    let label = match manager.get_webview_label(window.label(), &tab_id) {
        Some(l) => l,
        None => return CommandResult::err(format!("No webview for tab: {}", tab_id)),
    };

    let webview = match app.get_webview(&label) {
        Some(w) => w,
        None => return CommandResult::err(format!("Webview not found: {}", label)),
    };

    match webview.eval("history.forward()") {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(format!("Forward navigation failed: {}", e)),
    }
}

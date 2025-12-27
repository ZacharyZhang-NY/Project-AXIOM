use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, State, WebviewUrl, Window};

use super::tabs::{CommandResult, TabInfo};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct NewWindowInfo {
    pub window_label: String,
    pub session_id: String,
    pub tab: Option<TabInfo>,
}

fn build_browser_window(app: &AppHandle, window_label: &str) -> Result<(), String> {
    let window = WindowBuilder::new(app, window_label)
        .title("AXIOM")
        .inner_size(1280.0, 800.0)
        .min_inner_size(800.0, 600.0)
        .center()
        .build()
        .map_err(|e| e.to_string())?;

    let ui_webview = WebviewBuilder::new(
        super::ui_webview_label(window_label),
        WebviewUrl::App("index.html".into()),
    )
    .auto_resize()
    .enable_clipboard_access();

    let ui_webview = window
        .add_child(
            ui_webview,
            LogicalPosition::new(0.0, 0.0),
            LogicalSize::new(1280.0, 800.0),
        )
        .map_err(|e| e.to_string())?;

    let _ = ui_webview.show();

    Ok(())
}

#[tauri::command]
pub fn toggle_fullscreen(window: Window) -> CommandResult<bool> {
    let current = window.is_fullscreen().unwrap_or(false);
    let next = !current;
    match window.set_fullscreen(next) {
        Ok(()) => CommandResult::ok(next),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

fn next_window_label() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("window-{millis}")
}

#[tauri::command]
pub fn create_window(app: AppHandle, state: State<AppState>) -> CommandResult<NewWindowInfo> {
    let window_label = next_window_label();

    let session = match state.with_browser(|browser| browser.create_session("Window".to_string())) {
        Ok(s) => s,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let tab = match state.with_browser(|browser| {
        browser.create_tab_in_session(&session.id, "about:blank".to_string())
    }) {
        Ok(t) => t,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    state.set_session_for_window(&window_label, session.id.clone());

    if let Err(e) = build_browser_window(&app, &window_label) {
        return CommandResult::err(format!("Failed to create window: {e}"));
    }

    let _ = app.emit_to(super::ui_webview_label(&window_label), "tabs-updated", ());

    CommandResult::ok(NewWindowInfo {
        window_label,
        session_id: session.id,
        tab: Some(tab.into()),
    })
}

#[tauri::command]
pub fn open_url_in_new_window(
    app: AppHandle,
    state: State<AppState>,
    url: String,
) -> CommandResult<NewWindowInfo> {
    let window_label = next_window_label();

    let session = match state.with_browser(|browser| browser.create_session("Window".to_string())) {
        Ok(s) => s,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let tab = match state.with_browser(|browser| browser.create_tab_in_session(&session.id, url)) {
        Ok(t) => t,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    state.set_session_for_window(&window_label, session.id.clone());

    if let Err(e) = build_browser_window(&app, &window_label) {
        return CommandResult::err(format!("Failed to create window: {e}"));
    }

    let _ = app.emit_to(super::ui_webview_label(&window_label), "tabs-updated", ());

    CommandResult::ok(NewWindowInfo {
        window_label,
        session_id: session.id,
        tab: Some(tab.into()),
    })
}

#[tauri::command]
pub fn detach_tab_to_new_window(
    app: AppHandle,
    window: Window,
    state: State<AppState>,
    tab_id: String,
) -> CommandResult<NewWindowInfo> {
    let source_session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let tab = match state.with_browser(|browser| {
        browser
            .session_manager()
            .tab_manager()
            .get_tab(&tab_id)
            .map_err(Into::into)
    }) {
        Ok(t) => t,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    if let Err(e) =
        state.with_browser(|browser| browser.close_tab_in_session(&source_session_id, &tab_id))
    {
        return CommandResult::err(e.to_string());
    }

    let _ = app.emit_to(super::ui_webview_label(window.label()), "tabs-updated", ());

    let window_label = next_window_label();

    let session = match state.with_browser(|browser| browser.create_session("Window".to_string())) {
        Ok(s) => s,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    let new_tab =
        match state.with_browser(|browser| browser.create_tab_in_session(&session.id, tab.url)) {
            Ok(t) => t,
            Err(e) => return CommandResult::err(e.to_string()),
        };

    state.set_session_for_window(&window_label, session.id.clone());

    if let Err(e) = build_browser_window(&app, &window_label) {
        return CommandResult::err(format!("Failed to create window: {e}"));
    }

    let _ = app.emit_to(super::ui_webview_label(&window_label), "tabs-updated", ());

    CommandResult::ok(NewWindowInfo {
        window_label,
        session_id: session.id,
        tab: Some(new_tab.into()),
    })
}

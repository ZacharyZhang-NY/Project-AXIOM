//! Tab management commands
use serde::{Deserialize, Serialize};
use tauri::{State, Window};

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub favicon_url: Option<String>,
    pub state: String,
    pub is_loading: bool,
}

impl From<axiom_core::Tab> for TabInfo {
    fn from(tab: axiom_core::Tab) -> Self {
        let is_loading = tab.is_loading();
        Self {
            id: tab.id,
            url: tab.url,
            title: tab.title,
            favicon_url: tab.favicon_url,
            state: tab.state.as_str().to_string(),
            is_loading,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CommandResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> CommandResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(error: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error),
        }
    }
}

#[tauri::command]
pub fn create_tab(window: Window, state: State<AppState>, url: String) -> CommandResult<TabInfo> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.create_tab_in_session(&session_id, url)) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn create_tab_background(
    window: Window,
    state: State<AppState>,
    url: String,
) -> CommandResult<TabInfo> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.create_tab_in_session_background(&session_id, url)) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn close_tab(window: Window, state: State<AppState>, tab_id: String) -> CommandResult<()> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.close_tab_in_session(&session_id, &tab_id)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn restore_last_closed_tab(window: Window, state: State<AppState>) -> CommandResult<TabInfo> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.restore_last_closed_tab_in_session(&session_id)) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn activate_tab(
    window: Window,
    state: State<AppState>,
    tab_id: String,
) -> CommandResult<TabInfo> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.activate_tab_in_session(&session_id, &tab_id)) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_tabs(window: Window, state: State<AppState>) -> CommandResult<Vec<TabInfo>> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.get_ordered_tabs_in_session(&session_id)) {
        Ok(tabs) => CommandResult::ok(tabs.into_iter().map(TabInfo::from).collect()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_active_tab(window: Window, state: State<AppState>) -> CommandResult<Option<TabInfo>> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| browser.get_active_tab_in_session(&session_id)) {
        Ok(tab) => CommandResult::ok(tab.map(TabInfo::from)),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn navigate_tab(state: State<AppState>, tab_id: String, url: String) -> CommandResult<TabInfo> {
    match state.with_browser(|browser| browser.navigate_tab(&tab_id, url)) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_tab_title(
    state: State<AppState>,
    tab_id: String,
    title: String,
) -> CommandResult<TabInfo> {
    match state.with_browser(|browser| browser.set_tab_title(&tab_id, title)) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_tab_favicon(
    state: State<AppState>,
    tab_id: String,
    favicon_url: Option<String>,
) -> CommandResult<TabInfo> {
    match state.with_browser(|browser| {
        browser
            .session_manager()
            .tab_manager()
            .set_tab_favicon(&tab_id, favicon_url)
            .map_err(Into::into)
    }) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn reorder_tab(
    window: Window,
    state: State<AppState>,
    tab_id: String,
    new_index: usize,
) -> CommandResult<()> {
    let session_id = match state.session_id_for_window(window.label()) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state
        .with_browser(|browser| browser.reorder_tab_in_session(&session_id, &tab_id, new_index))
    {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn freeze_tab(state: State<AppState>, tab_id: String) -> CommandResult<TabInfo> {
    match state.with_browser(|browser| {
        browser
            .session_manager()
            .tab_manager()
            .freeze_tab(&tab_id)
            .map_err(Into::into)
    }) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn discard_tab(state: State<AppState>, tab_id: String) -> CommandResult<TabInfo> {
    match state.with_browser(|browser| {
        browser
            .session_manager()
            .tab_manager()
            .discard_tab(&tab_id)
            .map_err(Into::into)
    }) {
        Ok(tab) => CommandResult::ok(tab.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

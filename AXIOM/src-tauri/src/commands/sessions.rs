//! Session management commands
use serde::{Deserialize, Serialize};
use tauri::{State, Window};

use super::tabs::CommandResult;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub tab_count: usize,
}

impl SessionInfo {
    fn from_session(session: axiom_core::Session, is_active: bool) -> Self {
        let tab_count = session.tab_count();
        Self {
            id: session.id,
            name: session.name,
            is_active,
            tab_count,
        }
    }
}

#[tauri::command]
pub fn get_sessions(window: Window, state: State<AppState>) -> CommandResult<Vec<SessionInfo>> {
    let window_label = window.label();
    let active_id = state.session_id_for_window(window_label).ok();

    match state.with_browser(|browser| Ok(browser.list_sessions())) {
        Ok(sessions) => CommandResult::ok(
            sessions
                .into_iter()
                .map(|s| {
                    let is_active = active_id.as_deref() == Some(s.id.as_str());
                    SessionInfo::from_session(s, is_active)
                })
                .collect(),
        ),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_active_session(window: Window, state: State<AppState>) -> CommandResult<SessionInfo> {
    let window_label = window.label().to_string();
    let session_id = match state.session_id_for_window(&window_label) {
        Ok(id) => id,
        Err(e) => return CommandResult::err(e.to_string()),
    };

    match state.with_browser(|browser| {
        browser
            .session_manager()
            .get_session(&session_id)
            .map_err(Into::into)
    }) {
        Ok(session) => CommandResult::ok(SessionInfo::from_session(session, true)),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn create_session(state: State<AppState>, name: String) -> CommandResult<SessionInfo> {
    match state.with_browser(|browser| browser.create_session(name)) {
        Ok(session) => CommandResult::ok(SessionInfo::from_session(session, false)),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn switch_session(
    window: Window,
    state: State<AppState>,
    session_id: String,
) -> CommandResult<SessionInfo> {
    state.set_session_for_window(window.label(), session_id.clone());

    match state.with_browser(|browser| {
        browser
            .session_manager()
            .load_tabs_for_session(&session_id)?;
        browser
            .session_manager()
            .get_session(&session_id)
            .map_err(Into::into)
    }) {
        Ok(session) => CommandResult::ok(SessionInfo::from_session(session, true)),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn rename_session(
    state: State<AppState>,
    session_id: String,
    name: String,
) -> CommandResult<SessionInfo> {
    match state.with_browser(|browser| {
        browser
            .session_manager()
            .rename_session(&session_id, name)
            .map_err(Into::into)
    }) {
        Ok(session) => CommandResult::ok(SessionInfo::from_session(session, false)),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn delete_session(state: State<AppState>, session_id: String) -> CommandResult<()> {
    match state.with_browser(|browser| {
        browser
            .session_manager()
            .delete_session(&session_id)
            .map_err(Into::into)
    }) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

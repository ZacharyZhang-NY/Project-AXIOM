//! Settings commands

use serde::{Deserialize, Serialize};
use tauri::State;

use super::tabs::CommandResult;
use crate::state::AppState;
use axiom_core::Bookmark;

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsInfo {
    pub search_engine: String,
    pub theme: Option<String>,
    pub bookmarks_bar_visible: bool,
    pub autofill_enabled: bool,
    pub autofill_name: Option<String>,
    pub autofill_email: Option<String>,
    pub password_save_prompt_enabled: bool,
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> CommandResult<SettingsInfo> {
    match state.with_browser(|browser| {
        Ok(SettingsInfo {
            search_engine: browser.get_search_engine(),
            theme: browser.get_theme()?,
            bookmarks_bar_visible: browser.get_bookmarks_bar_visible()?,
            autofill_enabled: browser.get_autofill_enabled()?,
            autofill_name: browser.get_autofill_name()?,
            autofill_email: browser.get_autofill_email()?,
            password_save_prompt_enabled: browser.get_password_save_prompt_enabled()?,
        })
    }) {
        Ok(settings) => CommandResult::ok(settings),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_search_engine(state: State<AppState>, engine: String) -> CommandResult<()> {
    let template = match engine.to_lowercase().as_str() {
        "google" => "https://www.google.com/search?q=%s",
        "bing" => "https://www.bing.com/search?q=%s",
        "duckduckgo" => "https://duckduckgo.com/?q=%s",
        _ => return CommandResult::err("Unsupported search engine".to_string()),
    };

    match state.with_browser(|browser| browser.set_search_engine(template.to_string())) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_theme(state: State<AppState>, theme: String) -> CommandResult<()> {
    let normalized = theme.to_lowercase();
    if normalized != "light" && normalized != "dark" {
        return CommandResult::err("Unsupported theme".to_string());
    }

    match state.with_browser(|browser| browser.set_theme(normalized)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_bookmarks(state: State<AppState>) -> CommandResult<Vec<Bookmark>> {
    match state.with_browser(|browser| browser.get_bookmarks()) {
        Ok(bookmarks) => CommandResult::ok(bookmarks),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn add_bookmark(
    state: State<AppState>,
    title: String,
    url: String,
    folder: Option<String>,
) -> CommandResult<Vec<Bookmark>> {
    match state.with_browser(|browser| browser.add_bookmark(title, url, folder)) {
        Ok(bookmarks) => CommandResult::ok(bookmarks),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn remove_bookmark(state: State<AppState>, url: String) -> CommandResult<Vec<Bookmark>> {
    match state.with_browser(|browser| browser.remove_bookmark(&url)) {
        Ok(bookmarks) => CommandResult::ok(bookmarks),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn update_bookmark(
    state: State<AppState>,
    old_url: String,
    title: String,
    url: String,
    folder: Option<String>,
) -> CommandResult<Vec<Bookmark>> {
    match state.with_browser(|browser| browser.update_bookmark(&old_url, title, url, folder)) {
        Ok(bookmarks) => CommandResult::ok(bookmarks),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_bookmark_folders(state: State<AppState>) -> CommandResult<Vec<String>> {
    match state.with_browser(|browser| browser.get_bookmark_folders()) {
        Ok(folders) => CommandResult::ok(folders),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn export_bookmarks_html(state: State<AppState>) -> CommandResult<String> {
    match state.with_browser(|browser| browser.export_bookmarks_html()) {
        Ok(html) => CommandResult::ok(html),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn import_bookmarks_html(state: State<AppState>, html: String) -> CommandResult<Vec<Bookmark>> {
    match state.with_browser(|browser| browser.import_bookmarks_html(&html)) {
        Ok(bookmarks) => CommandResult::ok(bookmarks),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_bookmarks_bar_visibility(state: State<AppState>, visible: bool) -> CommandResult<()> {
    match state.with_browser(|browser| browser.set_bookmarks_bar_visible(visible)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_autofill_enabled(state: State<AppState>, enabled: bool) -> CommandResult<()> {
    match state.with_browser(|browser| browser.set_autofill_enabled(enabled)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_autofill_profile(
    state: State<AppState>,
    name: Option<String>,
    email: Option<String>,
) -> CommandResult<()> {
    match state.with_browser(|browser| browser.set_autofill_profile(name, email)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_password_save_prompt_enabled(
    state: State<AppState>,
    enabled: bool,
) -> CommandResult<()> {
    match state.with_browser(|browser| browser.set_password_save_prompt_enabled(enabled)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

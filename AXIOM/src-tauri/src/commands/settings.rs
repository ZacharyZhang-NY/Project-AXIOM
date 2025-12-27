//! Settings commands

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, Theme, Window};

use super::tabs::CommandResult;
use super::webview::WebviewManager;
use crate::state::AppState;
use axiom_core::Bookmark;

const FORCE_DARK_STYLE_ID: &str = "axiom-force-dark";
const FORCE_DARK_ENABLE_SCRIPT: &str = r#"
(() => {
  try {
    const id = '__AXIOM_FORCE_DARK_STYLE_ID__';
    const isTransparent = (value) => {
      if (!value) return true;
      const v = String(value).toLowerCase().replace(/\s+/g, '');
      return v === 'transparent' || v === 'rgba(0,0,0,0)' || v === 'rgba(0,0,0,0.0)';
    };
    const parseRgb = (value) => {
      const m = String(value).match(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i);
      if (!m) return null;
      return [Number(m[1]), Number(m[2]), Number(m[3])];
    };
    const brightness = (rgb) => (rgb[0] * 299 + rgb[1] * 587 + rgb[2] * 114) / 1000;
    const getBg = (el) => {
      try {
        return el ? getComputedStyle(el).backgroundColor : null;
      } catch {
        return null;
      }
    };

    const candidates = [];
    candidates.push(getBg(document.body));
    if (document.body) {
      candidates.push(getBg(document.body.firstElementChild));
      candidates.push(getBg(document.body.firstElementChild?.firstElementChild));
      candidates.push(getBg(document.querySelector('main')));
    }

    let bgRgb = null;
    for (const bg of candidates) {
      if (isTransparent(bg)) continue;
      const rgb = parseRgb(bg);
      if (rgb) {
        bgRgb = rgb;
        break;
      }
    }
    const looksDark = bgRgb ? brightness(bgRgb) < 140 : false;

    let style = document.getElementById(id);
    if (looksDark) {
      if (style) style.remove();
      try { document.documentElement.style.colorScheme = 'dark'; } catch {}
      return;
    }

    if (!style) {
      style = document.createElement('style');
      style.id = id;
      style.textContent = `
html { filter: invert(1) hue-rotate(180deg) !important; background: #111 !important; }
img, video, picture, canvas, iframe, svg { filter: invert(1) hue-rotate(180deg) !important; }
`;
      document.documentElement.appendChild(style);
    }
    try { document.documentElement.style.colorScheme = 'dark'; } catch {}       
  } catch {}
})();
"#;

const FORCE_DARK_DISABLE_SCRIPT: &str = r#"
(() => {
  try {
    const id = '__AXIOM_FORCE_DARK_STYLE_ID__';
    const style = document.getElementById(id);
    if (style) style.remove();
    try { document.documentElement.style.colorScheme = 'light'; } catch {}
  } catch {}
})();
"#;

fn force_dark_script(enabled: bool) -> String {
    let template = if enabled {
        FORCE_DARK_ENABLE_SCRIPT
    } else {
        FORCE_DARK_DISABLE_SCRIPT
    };
    template.replace("__AXIOM_FORCE_DARK_STYLE_ID__", FORCE_DARK_STYLE_ID)
}

fn apply_force_dark_to_webviews(app: &AppHandle, window_label: &str, enabled: bool) {
    let Some(manager) = app.try_state::<WebviewManager>() else {
        return;
    };

    let script = force_dark_script(enabled);
    for label in manager.get_all_labels(window_label) {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.eval(&script);
        }
    }
}

pub(crate) fn platform_theme_for(ui_theme: &str) -> Option<Theme> {
    // On Windows, the native title bar theme appears inverted relative to the requested theme.
    // Swap it so the window chrome matches the app theme.
    #[cfg(windows)]
    {
        match ui_theme {
            "dark" => Some(Theme::Light),
            "light" => Some(Theme::Dark),
            _ => None,
        }
    }

    #[cfg(not(windows))]
    {
        match ui_theme {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }
}

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
pub fn set_theme(
    app: AppHandle,
    window: Window,
    state: State<AppState>,
    theme: String,
) -> CommandResult<()> {
    let normalized = theme.to_lowercase();
    if normalized != "light" && normalized != "dark" {
        return CommandResult::err("Unsupported theme".to_string());
    }

    let platform_theme = platform_theme_for(normalized.as_str());

    app.set_theme(platform_theme);
    let _ = window.set_theme(platform_theme);
    apply_force_dark_to_webviews(&app, window.label(), normalized == "dark");

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

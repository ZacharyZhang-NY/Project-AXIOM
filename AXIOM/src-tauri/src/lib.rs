//! AXIOM Browser - Tauri Application
//!
//! Per PRD Section 7:
//! - Native UI per platform
//! - WebView is content only
//! - Rust owns all state

mod commands;
mod state;

use commands::downloads::DownloadRuntime;
use commands::webview::WebviewManager;
use state::AppState;
use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{LogicalPosition, LogicalSize, Manager, WebviewUrl};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    axiom_core::init_logging();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize browser state
            let state = AppState::new()?;
            state.initialize()?;

            let initial_theme = state
                .with_browser(|browser| browser.get_theme())
                .ok()
                .flatten();

            // Store state in Tauri
            app.manage(state);

            // Initialize webview manager
            app.manage(WebviewManager::new());

            // Initialize download runtime
            app.manage(DownloadRuntime::default());

            let window_label = "main";

            let window = WindowBuilder::new(app, window_label)
                .title("AXIOM")
                .inner_size(1280.0, 800.0)
                .min_inner_size(800.0, 600.0)
                .center()
                .build()?;

            if let Some(theme) = initial_theme.as_deref() {
                let platform_theme = commands::settings::platform_theme_for(theme);
                app.handle().set_theme(platform_theme);
                let _ = window.set_theme(platform_theme);
            }

            let ui_webview = WebviewBuilder::new(
                commands::ui_webview_label(window_label),
                WebviewUrl::App("index.html".into()),
            )
            .auto_resize()
            .enable_clipboard_access();

            let ui_webview = window.add_child(
                ui_webview,
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(1280.0, 800.0),
            )?;
            let _ = ui_webview.show();

            tracing::info!("AXIOM Browser started");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Diagnostics
            commands::diagnostics::frontend_ready,
            // Window commands
            commands::windows::create_window,
            commands::windows::open_url_in_new_window,
            commands::windows::detach_tab_to_new_window,
            commands::windows::toggle_fullscreen,
            // Tab commands
            commands::tabs::create_tab,
            commands::tabs::create_tab_background,
            commands::tabs::close_tab,
            commands::tabs::restore_last_closed_tab,
            commands::tabs::activate_tab,
            commands::tabs::get_tabs,
            commands::tabs::get_active_tab,
            commands::tabs::navigate_tab,
            commands::tabs::set_tab_title,
            commands::tabs::set_tab_favicon,
            commands::tabs::reorder_tab,
            commands::tabs::freeze_tab,
            commands::tabs::discard_tab,
            // Session commands
            commands::sessions::get_sessions,
            commands::sessions::get_active_session,
            commands::sessions::create_session,
            commands::sessions::switch_session,
            commands::sessions::rename_session,
            commands::sessions::delete_session,
            // Navigation commands
            commands::navigation::resolve_input,
            commands::navigation::probe_url,
            commands::navigation::search_history,
            commands::navigation::get_recent_history,
            commands::navigation::clear_history_range,
            // Privacy commands
            commands::privacy::check_permission,
            commands::privacy::set_permission,
            commands::privacy::should_block_url,
            commands::privacy::clean_url,
            commands::privacy::refresh_filter_lists,
            // Settings commands
            commands::settings::get_settings,
            commands::settings::set_search_engine,
            commands::settings::set_theme,
            commands::settings::get_bookmarks,
            commands::settings::add_bookmark,
            commands::settings::remove_bookmark,
            commands::settings::update_bookmark,
            commands::settings::get_bookmark_folders,
            commands::settings::export_bookmarks_html,
            commands::settings::import_bookmarks_html,
            commands::settings::set_bookmarks_bar_visibility,
            commands::settings::set_autofill_enabled,
            commands::settings::set_autofill_profile,
            commands::settings::set_password_save_prompt_enabled,
            // Webview commands
            commands::webview::create_webview,
            commands::webview::navigate_webview,
            commands::webview::show_webview,
            commands::webview::hide_webview,
            commands::webview::close_webview,
            commands::webview::set_webview_bounds,
            commands::webview::update_all_webview_bounds,
            commands::webview::reload_webview,
            commands::webview::force_reload_webview,
            commands::webview::stop_webview_loading,
            commands::webview::webview_back,
            commands::webview::webview_forward,
            // Download commands
            commands::downloads::list_downloads,
            commands::downloads::create_download,
            commands::downloads::start_download,
            commands::downloads::pause_download,
            commands::downloads::resume_download,
            commands::downloads::cancel_download,
            commands::downloads::reveal_download,
            // Reader mode
            commands::reader::extract_reader,
        ])
        .run(tauri::generate_context!())
        .expect("error while running AXIOM browser");
}

//! Tauri IPC Commands
//!
//! These commands bridge the frontend to the Rust core.
//! Per PRD: "Rust owns all state. WebView is stateless."

pub mod diagnostics;
pub mod downloads;
pub mod navigation;
pub mod privacy;
pub mod reader;
pub mod sessions;
pub mod settings;
pub mod tabs;
pub mod webview;
pub mod windows;

pub fn ui_webview_label(window_label: &str) -> String {
    format!("ui-{window_label}")
}

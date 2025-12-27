//! AXIOM Core
//!
//! Central coordination layer for the AXIOM browser.
//! Per PRD Section 7: "Rust owns all state. WebView is stateless."

mod bookmarks;
mod browser;
mod config;
mod error;

pub use bookmarks::Bookmark;
pub use browser::Browser;
pub use config::Config;
pub use error::CoreError;

// Re-export core components
pub use axiom_download::{Download, DownloadError, DownloadManager, DownloadState, RiskLevel};
pub use axiom_navigation::{
    Command, CommandType, HistoryEntry, HistoryManager, InputResolution, InputResolver,
    NavigationError,
};
pub use axiom_privacy::{
    Permission, PermissionManager, PermissionState, PermissionType, TrackingProtection,
};
pub use axiom_session::{Session, SessionError, SessionManager};
pub use axiom_storage::{Database, StorageError};
pub use axiom_tabs::{Tab, TabError, TabManager, TabState};

pub type Result<T> = std::result::Result<T, CoreError>;

/// Initialize logging
pub fn init_logging() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt().with_env_filter(filter).with_target(true).init();
}

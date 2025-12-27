//! Core error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Storage error: {0}")]
    Storage(#[from] axiom_storage::StorageError),

    #[error("Tab error: {0}")]
    Tab(#[from] axiom_tabs::TabError),

    #[error("Session error: {0}")]
    Session(#[from] axiom_session::SessionError),

    #[error("Navigation error: {0}")]
    Navigation(#[from] axiom_navigation::NavigationError),

    #[error("Download error: {0}")]
    Download(#[from] axiom_download::DownloadError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Browser not initialized")]
    NotInitialized,
}

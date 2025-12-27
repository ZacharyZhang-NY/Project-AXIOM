//! Session error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    Storage(#[from] axiom_storage::StorageError),

    #[error("Tab error: {0}")]
    Tab(#[from] axiom_tabs::TabError),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No active session")]
    NoActiveSession,

    #[error("Session name cannot be empty")]
    EmptyName,

    #[error("Cannot delete the last session")]
    CannotDeleteLastSession,
}

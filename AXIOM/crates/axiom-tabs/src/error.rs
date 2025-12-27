//! Tab error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum TabError {
    #[error("Tab not found: {0}")]
    NotFound(String),

    #[error("Invalid state transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },

    #[error("Storage error: {0}")]
    Storage(#[from] axiom_storage::StorageError),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

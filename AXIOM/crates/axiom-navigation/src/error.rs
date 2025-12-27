//! Navigation error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NavigationError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    #[error("Storage error: {0}")]
    Storage(#[from] axiom_storage::StorageError),
}

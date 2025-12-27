//! Download error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Download not found: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    Storage(#[from] axiom_storage::StorageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Download cancelled")]
    Cancelled,

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

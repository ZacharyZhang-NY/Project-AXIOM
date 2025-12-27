//! AXIOM Download Manager
//!
//! Per PRD Section 5.5:
//! - Rust-implemented downloader
//! - Explicit user consent before download
//! - File hash calculation
//! - Resume support
//! - MIME-based risk warning

mod download;
mod error;
mod manager;

pub use download::{Download, DownloadState, RiskLevel};
pub use error::DownloadError;
pub use manager::DownloadManager;

pub type Result<T> = std::result::Result<T, DownloadError>;

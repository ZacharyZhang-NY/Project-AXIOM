//! AXIOM Storage Layer
//!
//! SQLite-based persistence for all browser state.
//! All writes are transactional per PRD requirements.

mod database;
mod error;
mod migrations;

pub use database::Database;
pub use error::StorageError;

pub type Result<T> = std::result::Result<T, StorageError>;

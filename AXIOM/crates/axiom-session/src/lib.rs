//! AXIOM Session Management
//!
//! Per PRD Section 5.2:
//! - A Session is a persistent container of tabs and UI state
//! - Sessions auto-save on any mutation
//! - Multiple named sessions allowed
//! - Browser crash must restore last session exactly
//! - Sessions are local-only (no cross-device sync)

mod error;
mod manager;
mod session;

pub use error::SessionError;
pub use manager::SessionManager;
pub use session::Session;

pub type Result<T> = std::result::Result<T, SessionError>;

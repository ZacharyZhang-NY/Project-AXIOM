//! AXIOM Tab Management
//!
//! Implements the vertical tab system per PRD Section 5.1.
//! Tabs are "objects, not disposable pages" - they persist and have clear state transitions.

mod error;
mod manager;
mod state;
mod tab;

pub use error::TabError;
pub use manager::TabManager;
pub use state::TabState;
pub use tab::Tab;

pub type Result<T> = std::result::Result<T, TabError>;

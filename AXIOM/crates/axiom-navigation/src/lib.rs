//! AXIOM Navigation System
//!
//! Per PRD Section 5.3:
//! - Address Bar Input Resolution:
//!   1. Valid URL → navigate
//!   2. Invalid URL → search
//!   3. `@command` → internal command mode
//!
//! Supported Commands (Phase 1):
//! - `@tabs` — fuzzy search open tabs
//! - `@history` — fuzzy search history
//! - `@sessions` — switch session

mod command;
mod error;
mod history;
mod input;

pub use command::{Command, CommandType};
pub use error::NavigationError;
pub use history::{HistoryEntry, HistoryManager};
pub use input::{InputResolution, InputResolver};

pub type Result<T> = std::result::Result<T, NavigationError>;

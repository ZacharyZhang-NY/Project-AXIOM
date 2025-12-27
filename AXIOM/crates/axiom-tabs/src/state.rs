//! Tab State Machine
//!
//! Per PRD Section 5.1:
//! ```text
//! Active
//!   ↓ blur
//! Background
//!   ↓ idle timeout
//! Frozen
//!   ↓ memory pressure
//! Discarded
//! ```

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TabState {
    /// Tab is currently visible and active
    Active,
    /// Tab is loaded but not visible
    Background,
    /// Tab is suspended, JS execution stopped
    Frozen,
    /// Tab is unloaded, only URL and snapshot remain
    Discarded,
}

impl TabState {
    /// Check if transition to another state is valid
    pub fn can_transition_to(&self, target: TabState) -> bool {
        match (self, target) {
            // Active can go to Background (blur)
            (TabState::Active, TabState::Background) => true,
            // Background can go to Active (focus) or Frozen (idle timeout)
            (TabState::Background, TabState::Active) => true,
            (TabState::Background, TabState::Frozen) => true,
            // Frozen can go to Active (restore) or Discarded (memory pressure)
            (TabState::Frozen, TabState::Active) => true,
            (TabState::Frozen, TabState::Discarded) => true,
            // Discarded can go to Active (explicit restore by user click per PRD)
            (TabState::Discarded, TabState::Active) => true,
            // Same state is always valid (no-op)
            (a, b) if *a == b => true,
            // All other transitions are invalid
            _ => false,
        }
    }

    /// Returns true if JavaScript execution should be stopped
    pub fn should_freeze_js(&self) -> bool {
        matches!(self, TabState::Frozen | TabState::Discarded)
    }

    /// Returns true if the tab content is fully unloaded
    pub fn is_discarded(&self) -> bool {
        matches!(self, TabState::Discarded)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            TabState::Active => "active",
            TabState::Background => "background",
            TabState::Frozen => "frozen",
            TabState::Discarded => "discarded",
        }
    }
}

impl std::fmt::Display for TabState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for TabState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(TabState::Active),
            "background" => Ok(TabState::Background),
            "frozen" => Ok(TabState::Frozen),
            "discarded" => Ok(TabState::Discarded),
            _ => Err(format!("Unknown tab state: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        // Active -> Background
        assert!(TabState::Active.can_transition_to(TabState::Background));
        // Background -> Active
        assert!(TabState::Background.can_transition_to(TabState::Active));
        // Background -> Frozen
        assert!(TabState::Background.can_transition_to(TabState::Frozen));
        // Frozen -> Active
        assert!(TabState::Frozen.can_transition_to(TabState::Active));
        // Frozen -> Discarded
        assert!(TabState::Frozen.can_transition_to(TabState::Discarded));
        // Discarded -> Active (explicit restore)
        assert!(TabState::Discarded.can_transition_to(TabState::Active));
    }

    #[test]
    fn test_invalid_transitions() {
        // Can't go from Active directly to Frozen
        assert!(!TabState::Active.can_transition_to(TabState::Frozen));
        // Can't go from Active directly to Discarded
        assert!(!TabState::Active.can_transition_to(TabState::Discarded));
        // Can't go from Background directly to Discarded
        assert!(!TabState::Background.can_transition_to(TabState::Discarded));
    }
}

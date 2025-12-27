//! Tab data structure
//!
//! Per PRD Section 5.1, tabs display:
//! - Favicon
//! - Title (truncated)
//! - Loading / frozen indicator

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::TabError;
use crate::state::TabState;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tab {
    /// Unique identifier
    pub id: String,
    /// Session this tab belongs to
    pub session_id: String,
    /// Current URL
    pub url: String,
    /// Page title (may be truncated for display)
    pub title: String,
    /// Favicon URL if available
    pub favicon_url: Option<String>,
    /// Current state in the state machine
    pub state: TabState,
    /// Scroll position for restoration
    pub scroll_position: i32,
    /// When the tab was created
    pub created_at: DateTime<Utc>,
    /// Last modification time
    pub updated_at: DateTime<Utc>,
    /// Last time the tab was accessed/viewed
    pub last_accessed_at: DateTime<Utc>,
    /// Path to snapshot image for discarded tabs
    pub snapshot_path: Option<String>,
}

impl Tab {
    pub fn new(session_id: String, url: String) -> Result<Self> {
        // Validate URL
        if url.is_empty() {
            return Err(TabError::InvalidUrl("URL cannot be empty".to_string()));
        }

        let now = Utc::now();

        Ok(Self {
            id: Uuid::new_v4().to_string(),
            session_id,
            url,
            title: String::new(),
            favicon_url: None,
            state: TabState::Active,
            scroll_position: 0,
            created_at: now,
            updated_at: now,
            last_accessed_at: now,
            snapshot_path: None,
        })
    }

    /// Attempt to transition to a new state
    pub fn transition_to(&mut self, new_state: TabState) -> Result<()> {
        if !self.state.can_transition_to(new_state) {
            return Err(TabError::InvalidTransition {
                from: self.state.to_string(),
                to: new_state.to_string(),
            });
        }

        tracing::debug!(
            tab_id = %self.id,
            from = %self.state,
            to = %new_state,
            "Tab state transition"
        );

        self.state = new_state;
        self.updated_at = Utc::now();

        if new_state == TabState::Active {
            self.last_accessed_at = Utc::now();
        }

        Ok(())
    }

    /// Mark tab as active (user selected it)
    pub fn activate(&mut self) -> Result<()> {
        self.transition_to(TabState::Active)
    }

    /// Move tab to background (blur)
    pub fn blur(&mut self) -> Result<()> {
        if self.state == TabState::Active {
            self.transition_to(TabState::Background)
        } else {
            Ok(()) // Already not active
        }
    }

    /// Freeze the tab (stop JS execution)
    pub fn freeze(&mut self) -> Result<()> {
        if self.state == TabState::Background {
            self.transition_to(TabState::Frozen)
        } else if self.state == TabState::Active {
            // First blur, then freeze
            self.blur()?;
            self.transition_to(TabState::Frozen)
        } else {
            Ok(()) // Already frozen or discarded
        }
    }

    /// Discard the tab (unload content, keep snapshot)
    pub fn discard(&mut self) -> Result<()> {
        if self.state == TabState::Frozen {
            self.transition_to(TabState::Discarded)
        } else if self.state != TabState::Discarded {
            // Must freeze first
            self.freeze()?;
            self.transition_to(TabState::Discarded)
        } else {
            Ok(())
        }
    }

    /// Update page title
    pub fn set_title(&mut self, title: String) {
        self.title = title;
        self.updated_at = Utc::now();
    }

    /// Update favicon
    pub fn set_favicon(&mut self, url: Option<String>) {
        self.favicon_url = url;
        self.updated_at = Utc::now();
    }

    /// Update URL (navigation)
    pub fn navigate(&mut self, url: String) -> Result<()> {
        if url.is_empty() {
            return Err(TabError::InvalidUrl("URL cannot be empty".to_string()));
        }

        self.url = url;
        self.title = String::new(); // Reset title until page loads
        self.scroll_position = 0;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Check if tab is loading content
    pub fn is_loading(&self) -> bool {
        // For now, we consider a tab "loading" if it's active but has no title yet
        self.state == TabState::Active && self.title.is_empty()
    }

    /// Get display title (with fallback to URL)
    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            &self.url
        } else {
            &self.title
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tab() {
        let tab = Tab::new("session-1".to_string(), "https://example.com".to_string()).unwrap();
        assert_eq!(tab.state, TabState::Active);
        assert_eq!(tab.url, "https://example.com");
        assert!(tab.title.is_empty());
    }

    #[test]
    fn test_state_transitions() {
        let mut tab = Tab::new("session-1".to_string(), "https://example.com".to_string()).unwrap();

        // Active -> Background
        tab.blur().unwrap();
        assert_eq!(tab.state, TabState::Background);

        // Background -> Frozen
        tab.freeze().unwrap();
        assert_eq!(tab.state, TabState::Frozen);

        // Frozen -> Discarded
        tab.discard().unwrap();
        assert_eq!(tab.state, TabState::Discarded);

        // Discarded -> Active (restore)
        tab.activate().unwrap();
        assert_eq!(tab.state, TabState::Active);
    }

    #[test]
    fn test_empty_url_rejected() {
        let result = Tab::new("session-1".to_string(), String::new());
        assert!(result.is_err());
    }
}

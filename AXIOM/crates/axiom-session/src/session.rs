//! Session data structure

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// When the session was created
    pub created_at: DateTime<Utc>,
    /// Last modification time
    pub updated_at: DateTime<Utc>,
    /// Whether this is the currently active session
    pub is_active: bool,
    /// Ordered list of tab IDs (for display order in sidebar)
    pub tab_order: Vec<String>,
}

impl Session {
    pub fn new(name: String) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4().to_string(),
            name,
            created_at: now,
            updated_at: now,
            is_active: false,
            tab_order: Vec::new(),
        }
    }

    /// Create a default session for new users
    pub fn default_session() -> Self {
        let mut session = Self::new("Default".to_string());
        session.is_active = true;
        session
    }

    /// Add a tab ID to the order list
    pub fn add_tab(&mut self, tab_id: String) {
        if !self.tab_order.contains(&tab_id) {
            self.tab_order.push(tab_id);
            self.updated_at = Utc::now();
        }
    }

    /// Remove a tab ID from the order list
    pub fn remove_tab(&mut self, tab_id: &str) {
        self.tab_order.retain(|id| id != tab_id);
        self.updated_at = Utc::now();
    }

    /// Move a tab to a new position
    pub fn move_tab(&mut self, tab_id: &str, new_index: usize) {
        if let Some(current_index) = self.tab_order.iter().position(|id| id == tab_id) {
            let tab_id = self.tab_order.remove(current_index);
            let insert_index = new_index.min(self.tab_order.len());
            self.tab_order.insert(insert_index, tab_id);
            self.updated_at = Utc::now();
        }
    }

    /// Rename the session
    pub fn rename(&mut self, name: String) {
        self.name = name;
        self.updated_at = Utc::now();
    }

    /// Get the number of tabs
    pub fn tab_count(&self) -> usize {
        self.tab_order.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session() {
        let session = Session::new("Work".to_string());
        assert_eq!(session.name, "Work");
        assert!(!session.is_active);
        assert!(session.tab_order.is_empty());
    }

    #[test]
    fn test_tab_order() {
        let mut session = Session::new("Test".to_string());

        session.add_tab("tab-1".to_string());
        session.add_tab("tab-2".to_string());
        session.add_tab("tab-3".to_string());

        assert_eq!(session.tab_order, vec!["tab-1", "tab-2", "tab-3"]);

        // Move tab-3 to the beginning
        session.move_tab("tab-3", 0);
        assert_eq!(session.tab_order, vec!["tab-3", "tab-1", "tab-2"]);

        // Remove tab-1
        session.remove_tab("tab-1");
        assert_eq!(session.tab_order, vec!["tab-3", "tab-2"]);
    }
}

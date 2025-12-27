//! Tab Manager
//!
//! Manages all tabs across sessions with persistence.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use axiom_storage::Database;

use crate::error::TabError;
use crate::state::TabState;
use crate::tab::Tab;
use crate::Result;

pub struct TabManager {
    /// In-memory tab cache
    tabs: Arc<RwLock<HashMap<String, Tab>>>,
    /// Database for persistence
    db: Database,
}

impl TabManager {
    pub fn new(db: Database) -> Self {
        Self {
            tabs: Arc::new(RwLock::new(HashMap::new())),
            db,
        }
    }

    /// Load all tabs for a session from database
    pub fn load_session_tabs(&self, session_id: &str) -> Result<Vec<Tab>> {
        let tabs: Vec<Tab> = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, url, title, favicon_url, state, scroll_position,
                        created_at, updated_at, last_accessed_at, snapshot_path
                 FROM tabs WHERE session_id = ?1",
            )?;

            let tabs: Vec<Tab> = stmt
                .query_map([session_id], |row| {
                    let state_str: String = row.get(5)?;
                    let state: TabState = state_str.parse().unwrap_or(TabState::Background);

                    // Parse datetime strings
                    let created_str: String = row.get(7)?;
                    let updated_str: String = row.get(8)?;
                    let accessed_str: String = row.get(9)?;

                    let created_at = DateTime::parse_from_rfc3339(&created_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());
                    let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());
                    let last_accessed_at = DateTime::parse_from_rfc3339(&accessed_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(Tab {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        url: row.get(2)?,
                        title: row.get(3)?,
                        favicon_url: row.get(4)?,
                        state,
                        scroll_position: row.get(6)?,
                        created_at,
                        updated_at,
                        last_accessed_at,
                        snapshot_path: row.get(10)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(tabs)
        })?;

        // Cache in memory
        {
            let mut cache = self.tabs.write();
            for tab in &tabs {
                cache.insert(tab.id.clone(), tab.clone());
            }
        }

        Ok(tabs)
    }

    /// Create a new tab
    pub fn create_tab(&self, session_id: String, url: String) -> Result<Tab> {
        let tab = Tab::new(session_id, url)?;

        // Persist to database
        self.save_tab(&tab)?;

        // Add to cache
        self.tabs.write().insert(tab.id.clone(), tab.clone());

        tracing::info!(tab_id = %tab.id, url = %tab.url, "Created new tab");

        Ok(tab)
    }

    /// Get a tab by ID
    pub fn get_tab(&self, tab_id: &str) -> Result<Tab> {
        self.tabs
            .read()
            .get(tab_id)
            .cloned()
            .ok_or_else(|| TabError::NotFound(tab_id.to_string()))
    }

    /// Update a tab
    pub fn update_tab(&self, tab: &Tab) -> Result<()> {
        self.save_tab(tab)?;
        self.tabs.write().insert(tab.id.clone(), tab.clone());
        Ok(())
    }

    /// Activate a tab (set as current)
    pub fn activate_tab(&self, tab_id: &str) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.activate()?;
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Blur a tab (move to background)
    pub fn blur_tab(&self, tab_id: &str) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.blur()?;
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Freeze a tab
    pub fn freeze_tab(&self, tab_id: &str) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.freeze()?;
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Discard a tab
    pub fn discard_tab(&self, tab_id: &str) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.discard()?;
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Close a tab (remove from session)
    pub fn close_tab(&self, tab_id: &str) -> Result<()> {
        // Remove from database
        self.db.with_connection(|conn| {
            conn.execute("DELETE FROM tabs WHERE id = ?1", [tab_id])?;
            Ok(())
        })?;

        // Remove from cache
        self.tabs.write().remove(tab_id);

        tracing::info!(tab_id = %tab_id, "Closed tab");

        Ok(())
    }

    /// Get all tabs in a session
    pub fn get_session_tabs(&self, session_id: &str) -> Vec<Tab> {
        self.tabs
            .read()
            .values()
            .filter(|t| t.session_id == session_id)
            .cloned()
            .collect()
    }

    /// Navigate a tab to a new URL
    pub fn navigate_tab(&self, tab_id: &str, url: String) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.navigate(url)?;
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Update tab title
    pub fn set_tab_title(&self, tab_id: &str, title: String) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.set_title(title);
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Update tab favicon
    pub fn set_tab_favicon(&self, tab_id: &str, favicon_url: Option<String>) -> Result<Tab> {
        let mut tab = self.get_tab(tab_id)?;
        tab.set_favicon(favicon_url);
        self.update_tab(&tab)?;
        Ok(tab)
    }

    /// Save tab to database
    fn save_tab(&self, tab: &Tab) -> Result<()> {
        Ok(self.db.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO tabs
                 (id, session_id, url, title, favicon_url, state, scroll_position,
                  created_at, updated_at, last_accessed_at, snapshot_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    tab.id,
                    tab.session_id,
                    tab.url,
                    tab.title,
                    tab.favicon_url,
                    tab.state.as_str(),
                    tab.scroll_position,
                    tab.created_at.to_rfc3339(),
                    tab.updated_at.to_rfc3339(),
                    tab.last_accessed_at.to_rfc3339(),
                    tab.snapshot_path,
                ],
            )?;
            Ok(())
        })?)
    }
}

impl Clone for TabManager {
    fn clone(&self) -> Self {
        Self {
            tabs: Arc::clone(&self.tabs),
            db: self.db.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_manager() {
        let db = Database::open_in_memory().unwrap();

        // Create a session first (required by foreign key constraint)
        db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO sessions (id, name, created_at, updated_at, is_active, tab_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    "session-1",
                    "Test Session",
                    chrono::Utc::now().to_rfc3339(),
                    chrono::Utc::now().to_rfc3339(),
                    1,
                    "[]"
                ],
            )?;
            Ok(())
        })
        .unwrap();

        let manager = TabManager::new(db);

        // Create a tab
        let tab = manager
            .create_tab("session-1".to_string(), "https://example.com".to_string())
            .unwrap();

        assert_eq!(tab.state, TabState::Active);

        // Get the tab
        let retrieved = manager.get_tab(&tab.id).unwrap();
        assert_eq!(retrieved.url, "https://example.com");

        // Blur the tab
        let blurred = manager.blur_tab(&tab.id).unwrap();
        assert_eq!(blurred.state, TabState::Background);

        // Close the tab
        manager.close_tab(&tab.id).unwrap();
        assert!(manager.get_tab(&tab.id).is_err());
    }
}

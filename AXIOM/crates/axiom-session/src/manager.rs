//! Session Manager
//!
//! Handles session persistence and restoration.
//! Per PRD: "Sessions auto-save on any mutation"

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use axiom_storage::Database;
use axiom_tabs::{Tab, TabManager};

use crate::error::SessionError;
use crate::session::Session;
use crate::Result;

pub struct SessionManager {
    /// In-memory session cache
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    /// Currently active session ID
    active_session_id: Arc<RwLock<Option<String>>>,
    /// Database for persistence
    db: Database,
    /// Tab manager
    tab_manager: TabManager,
}

impl SessionManager {
    pub fn new(db: Database) -> Self {
        let tab_manager = TabManager::new(db.clone());

        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            active_session_id: Arc::new(RwLock::new(None)),
            db,
            tab_manager,
        }
    }

    /// Initialize and load sessions from database
    /// Returns the active session or creates a default one
    pub fn initialize(&self) -> Result<Session> {
        // Load all sessions
        let sessions = self.load_all_sessions()?;

        // Find active session or create default
        let active_session = sessions
            .iter()
            .find(|s| s.is_active)
            .cloned()
            .unwrap_or_else(|| {
                let session = Session::default_session();
                // Save the new default session
                if let Err(e) = self.save_session(&session) {
                    tracing::error!("Failed to save default session: {}", e);
                }
                session
            });

        // Set as active
        *self.active_session_id.write() = Some(active_session.id.clone());

        // Load tabs for active session
        self.tab_manager.load_session_tabs(&active_session.id)?;

        tracing::info!(
            session_id = %active_session.id,
            session_name = %active_session.name,
            tab_count = active_session.tab_count(),
            "Initialized session"
        );

        Ok(active_session)
    }

    /// Load all sessions from database
    fn load_all_sessions(&self) -> Result<Vec<Session>> {
        let sessions = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, created_at, updated_at, is_active, tab_order FROM sessions",
            )?;

            let sessions: Vec<Session> = stmt
                .query_map([], |row| {
                    let tab_order_json: String = row.get(5)?;
                    let tab_order: Vec<String> =
                        serde_json::from_str(&tab_order_json).unwrap_or_default();

                    // Parse datetime strings
                    let created_str: String = row.get(2)?;
                    let updated_str: String = row.get(3)?;

                    let created_at = DateTime::parse_from_rfc3339(&created_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());
                    let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(Session {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        created_at,
                        updated_at,
                        is_active: row.get::<_, i32>(4)? != 0,
                        tab_order,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(sessions)
        })?;

        // Cache in memory
        {
            let mut cache = self.sessions.write();
            for session in &sessions {
                cache.insert(session.id.clone(), session.clone());
            }
        }

        Ok(sessions)
    }

    /// Save session to database (auto-save on mutation)
    fn save_session(&self, session: &Session) -> Result<()> {
        let tab_order_json = serde_json::to_string(&session.tab_order)?;

        self.db.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO sessions
                 (id, name, created_at, updated_at, is_active, tab_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    session.id,
                    session.name,
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                    session.is_active as i32,
                    tab_order_json,
                ],
            )?;
            Ok(())
        })?;

        // Update cache
        self.sessions
            .write()
            .insert(session.id.clone(), session.clone());

        Ok(())
    }

    /// Get the currently active session
    pub fn active_session(&self) -> Result<Session> {
        let active_id = self
            .active_session_id
            .read()
            .clone()
            .ok_or(SessionError::NoActiveSession)?;

        self.sessions
            .read()
            .get(&active_id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound(active_id))
    }

    pub fn get_session(&self, session_id: &str) -> Result<Session> {
        self.sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))
    }

    /// Create a new session
    pub fn create_session(&self, name: String) -> Result<Session> {
        if name.trim().is_empty() {
            return Err(SessionError::EmptyName);
        }

        let session = Session::new(name);
        self.save_session(&session)?;

        tracing::info!(
            session_id = %session.id,
            session_name = %session.name,
            "Created new session"
        );

        Ok(session)
    }

    /// Switch to a different session
    pub fn switch_session(&self, session_id: &str) -> Result<Session> {
        // Deactivate current session
        if let Ok(mut current) = self.active_session() {
            current.is_active = false;
            self.save_session(&current)?;
        }

        // Activate new session
        let mut session = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.is_active = true;
        self.save_session(&session)?;
        *self.active_session_id.write() = Some(session.id.clone());

        // Load tabs for new session
        self.tab_manager.load_session_tabs(&session.id)?;

        tracing::info!(
            session_id = %session.id,
            session_name = %session.name,
            "Switched to session"
        );

        Ok(session)
    }

    /// Get all sessions
    pub fn list_sessions(&self) -> Vec<Session> {
        self.sessions.read().values().cloned().collect()
    }

    /// Rename a session
    pub fn rename_session(&self, session_id: &str, name: String) -> Result<Session> {
        if name.trim().is_empty() {
            return Err(SessionError::EmptyName);
        }

        let mut session = self
            .sessions
            .read()
            .get(session_id)
            .cloned()
            .ok_or_else(|| SessionError::NotFound(session_id.to_string()))?;

        session.rename(name);
        self.save_session(&session)?;

        Ok(session)
    }

    /// Delete a session (cannot delete the last session)
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        let session_count = self.sessions.read().len();
        if session_count <= 1 {
            return Err(SessionError::CannotDeleteLastSession);
        }

        // If deleting active session, switch to another first
        {
            let active_id = self.active_session_id.read().clone();
            if active_id.as_deref() == Some(session_id) {
                // Find another session to switch to
                if let Some(other_session) = self
                    .sessions
                    .read()
                    .values()
                    .find(|s| s.id != session_id)
                    .cloned()
                {
                    self.switch_session(&other_session.id)?;
                }
            }
        }

        // Delete from database (cascades to tabs)
        self.db.with_connection(|conn| {
            conn.execute("DELETE FROM sessions WHERE id = ?1", [session_id])?;
            Ok(())
        })?;

        // Remove from cache
        self.sessions.write().remove(session_id);

        tracing::info!(session_id = %session_id, "Deleted session");

        Ok(())
    }

    /// Get the tab manager
    pub fn tab_manager(&self) -> &TabManager {
        &self.tab_manager
    }

    pub fn load_tabs_for_session(&self, session_id: &str) -> Result<()> {
        self.tab_manager.load_session_tabs(session_id)?;
        Ok(())
    }

    pub fn add_tab_to_session(&self, session_id: &str, tab_id: String) -> Result<Session> {
        let mut session = self.get_session(session_id)?;
        session.add_tab(tab_id);
        self.save_session(&session)?;
        Ok(session)
    }

    pub fn remove_tab_from_session(&self, session_id: &str, tab_id: &str) -> Result<Session> {
        let mut session = self.get_session(session_id)?;
        session.remove_tab(tab_id);
        self.save_session(&session)?;
        Ok(session)
    }

    pub fn move_tab_in_session(
        &self,
        session_id: &str,
        tab_id: &str,
        new_index: usize,
    ) -> Result<Session> {
        let mut session = self.get_session(session_id)?;
        session.move_tab(tab_id, new_index);
        self.save_session(&session)?;
        Ok(session)
    }

    pub fn get_ordered_tabs_for_session(&self, session_id: &str) -> Result<Vec<Tab>> {
        let session = self.get_session(session_id)?;
        let all_tabs = self.tab_manager.get_session_tabs(&session.id);

        let mut ordered: Vec<Tab> = Vec::with_capacity(session.tab_order.len());
        for tab_id in &session.tab_order {
            if let Some(tab) = all_tabs.iter().find(|t| &t.id == tab_id) {
                ordered.push(tab.clone());
            }
        }

        Ok(ordered)
    }

    /// Create a new tab in the active session
    pub fn create_tab(&self, url: String) -> Result<Tab> {
        let mut session = self.active_session()?;
        let tab = self.tab_manager.create_tab(session.id.clone(), url)?;

        session.add_tab(tab.id.clone());
        self.save_session(&session)?;

        Ok(tab)
    }

    /// Close a tab in the active session
    pub fn close_tab(&self, tab_id: &str) -> Result<()> {
        let mut session = self.active_session()?;

        self.tab_manager.close_tab(tab_id)?;
        session.remove_tab(tab_id);
        self.save_session(&session)?;

        Ok(())
    }

    /// Reorder tabs in the active session
    pub fn move_tab(&self, tab_id: &str, new_index: usize) -> Result<()> {
        let mut session = self.active_session()?;
        session.move_tab(tab_id, new_index);
        self.save_session(&session)?;

        Ok(())
    }

    /// Get all tabs in the active session, ordered
    pub fn get_ordered_tabs(&self) -> Result<Vec<Tab>> {
        let session = self.active_session()?;
        let all_tabs = self.tab_manager.get_session_tabs(&session.id);

        // Order tabs according to session.tab_order
        let mut ordered: Vec<Tab> = Vec::with_capacity(session.tab_order.len());
        for tab_id in &session.tab_order {
            if let Some(tab) = all_tabs.iter().find(|t| &t.id == tab_id) {
                ordered.push(tab.clone());
            }
        }

        Ok(ordered)
    }
}

impl Clone for SessionManager {
    fn clone(&self) -> Self {
        Self {
            sessions: Arc::clone(&self.sessions),
            active_session_id: Arc::clone(&self.active_session_id),
            db: self.db.clone(),
            tab_manager: self.tab_manager.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager() {
        let db = Database::open_in_memory().unwrap();
        let manager = SessionManager::new(db);

        // Initialize (creates default session)
        let session = manager.initialize().unwrap();
        assert!(session.is_active);
        assert_eq!(session.name, "Default");

        // Create another session
        let work_session = manager.create_session("Work".to_string()).unwrap();
        assert!(!work_session.is_active);

        // Switch to work session
        let switched = manager.switch_session(&work_session.id).unwrap();
        assert!(switched.is_active);
        assert_eq!(switched.name, "Work");

        // Verify original session is no longer active
        let sessions = manager.list_sessions();
        let default = sessions.iter().find(|s| s.name == "Default").unwrap();
        assert!(!default.is_active);
    }
}

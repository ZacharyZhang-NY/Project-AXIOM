//! Main browser state container
//!
//! Per PRD Section 7: "Rust owns all state. WebView is stateless."

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::sync::Arc;

use axiom_download::DownloadManager;
use axiom_navigation::{HistoryManager, InputResolver};
use axiom_privacy::{PermissionManager, TrackingProtection};
use axiom_session::SessionManager;
use axiom_storage::Database;

use crate::bookmarks::Bookmark;
use crate::config::Config;
use crate::error::CoreError;
use crate::Result;

#[derive(Debug, Clone)]
struct ClosedTab {
    session_id: String,
    url: String,
    title: String,
    favicon_url: Option<String>,
    index: usize,
}

/// Main browser instance
///
/// This is the central state container for the entire browser.
/// All state flows through here, and the WebView is purely a renderer.
pub struct Browser {
    /// Configuration
    config: Config,
    /// Database
    db: Database,
    /// Session manager (includes tab management)
    session_manager: SessionManager,
    /// History manager
    history_manager: HistoryManager,
    /// Input resolver for address bar
    input_resolver: Arc<RwLock<InputResolver>>,
    /// Download manager
    download_manager: DownloadManager,
    /// Permission manager
    permission_manager: Arc<RwLock<PermissionManager>>,
    /// Tracking protection
    tracking_protection: Arc<RwLock<TrackingProtection>>,
    /// Current active tab ID
    active_tab_id: Arc<RwLock<Option<String>>>,
    recently_closed_tabs: Arc<RwLock<Vec<ClosedTab>>>,
}

impl Browser {
    /// Initialize a new browser instance
    pub fn new(config: Config) -> Result<Self> {
        // Ensure data directory exists
        if let Some(parent) = config.database_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open database
        let db = Database::open(&config.database_path)?;

        // Initialize managers
        let session_manager = SessionManager::new(db.clone());
        let history_manager = HistoryManager::new(db.clone());
        let input_resolver = Arc::new(RwLock::new(InputResolver::with_search_engine(
            config.search_engine.clone(),
        )));
        let download_manager = DownloadManager::new(db.clone(), config.download_dir.clone());

        let mut tracking_protection = TrackingProtection::new();
        tracking_protection.set_enabled(config.tracking_protection);

        Ok(Self {
            config,
            db,
            session_manager,
            history_manager,
            input_resolver,
            download_manager,
            permission_manager: Arc::new(RwLock::new(PermissionManager::new())),
            tracking_protection: Arc::new(RwLock::new(tracking_protection)),
            active_tab_id: Arc::new(RwLock::new(None)),
            recently_closed_tabs: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Initialize browser state (load sessions, restore state)
    pub fn initialize(&self) -> Result<()> {
        // Initialize session manager (loads/creates default session)
        self.session_manager.initialize()?;

        // Load downloads
        self.download_manager.load_downloads()?;

        // Apply persisted search engine preference if available
        if let Some(template) = self.db.get_setting("search_engine")? {
            self.input_resolver.write().set_search_engine(template);
        }

        if let Some(domains_json) = self.db.get_setting("blocked_domains")? {
            if let Ok(domains) = serde_json::from_str::<Vec<String>>(&domains_json) {
                self.tracking_protection
                    .write()
                    .set_blocked_domains(domains);
            }
        }

        if let Some(perms_json) = self.db.get_setting("permissions")? {
            if let Ok(perms) = serde_json::from_str::<Vec<axiom_privacy::Permission>>(&perms_json) {
                self.permission_manager.write().import_permissions(perms);
            }
        }

        // Set active tab based on stored tab state (fallback to first in order)
        let ordered_tabs = self.session_manager.get_ordered_tabs()?;
        let active_tab_id = ordered_tabs
            .iter()
            .find(|tab| tab.state == axiom_tabs::TabState::Active)
            .map(|tab| tab.id.clone())
            .or_else(|| ordered_tabs.first().map(|tab| tab.id.clone()));

        *self.active_tab_id.write() = active_tab_id;

        tracing::info!("Browser initialized");

        Ok(())
    }

    // === Session operations ===

    pub fn session_manager(&self) -> &SessionManager {
        &self.session_manager
    }

    pub fn create_session(&self, name: String) -> Result<axiom_session::Session> {
        Ok(self.session_manager.create_session(name)?)
    }

    pub fn switch_session(&self, session_id: &str) -> Result<axiom_session::Session> {
        let session = self.session_manager.switch_session(session_id)?;

        // Update active tab
        if let Some(first_tab_id) = session.tab_order.first() {
            let tab = self
                .session_manager
                .tab_manager()
                .activate_tab(first_tab_id)?;
            *self.active_tab_id.write() = Some(tab.id);
        } else {
            *self.active_tab_id.write() = None;
        }

        Ok(session)
    }

    pub fn list_sessions(&self) -> Vec<axiom_session::Session> {
        self.session_manager.list_sessions()
    }

    // === Tab operations ===

    pub fn create_tab(&self, url: String) -> Result<axiom_tabs::Tab> {
        if let Some(current_id) = self.active_tab_id.read().as_ref() {
            let _ = self.session_manager.tab_manager().blur_tab(current_id);
        }

        let tab = self.session_manager.create_tab(url)?;
        *self.active_tab_id.write() = Some(tab.id.clone());
        Ok(tab)
    }

    pub fn close_tab(&self, tab_id: &str) -> Result<()> {
        if let Ok(session) = self.session_manager.active_session() {
            if let Ok(tab) = self.session_manager.tab_manager().get_tab(tab_id) {
                let index = session
                    .tab_order
                    .iter()
                    .position(|id| id == tab_id)
                    .unwrap_or(session.tab_order.len());

                let mut stack = self.recently_closed_tabs.write();
                stack.push(ClosedTab {
                    session_id: tab.session_id.clone(),
                    url: tab.url.clone(),
                    title: tab.title.clone(),
                    favicon_url: tab.favicon_url.clone(),
                    index,
                });

                if stack.len() > 20 {
                    let overflow = stack.len() - 20;
                    stack.drain(0..overflow);
                }
            }
        }

        self.session_manager.close_tab(tab_id)?;

        // If we closed the active tab, switch to another
        let active = self.active_tab_id.read().clone();
        if active.as_deref() == Some(tab_id) {
            let tabs = self.session_manager.get_ordered_tabs()?;
            if let Some(next_tab) = tabs.first() {
                let next_tab = self
                    .session_manager
                    .tab_manager()
                    .activate_tab(&next_tab.id)?;
                *self.active_tab_id.write() = Some(next_tab.id);
            } else {
                *self.active_tab_id.write() = None;
            }
        }

        Ok(())
    }

    pub fn activate_tab(&self, tab_id: &str) -> Result<axiom_tabs::Tab> {
        // Blur current tab
        if let Some(current_id) = self.active_tab_id.read().as_ref() {
            let _ = self.session_manager.tab_manager().blur_tab(current_id);
        }

        // Activate new tab
        let tab = self.session_manager.tab_manager().activate_tab(tab_id)?;
        *self.active_tab_id.write() = Some(tab_id.to_string());

        Ok(tab)
    }

    pub fn get_active_tab(&self) -> Result<Option<axiom_tabs::Tab>> {
        match self.active_tab_id.read().as_ref() {
            Some(id) => Ok(Some(self.session_manager.tab_manager().get_tab(id)?)),
            None => Ok(None),
        }
    }

    pub fn get_ordered_tabs(&self) -> Result<Vec<axiom_tabs::Tab>> {
        Ok(self.session_manager.get_ordered_tabs()?)
    }

    pub fn get_ordered_tabs_in_session(&self, session_id: &str) -> Result<Vec<axiom_tabs::Tab>> {
        Ok(self
            .session_manager
            .get_ordered_tabs_for_session(session_id)?)
    }

    pub fn get_active_tab_in_session(&self, session_id: &str) -> Result<Option<axiom_tabs::Tab>> {
        let tabs = self.get_ordered_tabs_in_session(session_id)?;
        Ok(tabs
            .into_iter()
            .find(|tab| tab.state == axiom_tabs::TabState::Active))
    }

    pub fn create_tab_in_session(&self, session_id: &str, url: String) -> Result<axiom_tabs::Tab> {
        self.session_manager.load_tabs_for_session(session_id)?;

        if let Some(active) = self.get_active_tab_in_session(session_id)? {
            let _ = self.session_manager.tab_manager().blur_tab(&active.id);
        }

        let tab = self
            .session_manager
            .tab_manager()
            .create_tab(session_id.to_string(), url)?;
        let _ = self
            .session_manager
            .add_tab_to_session(session_id, tab.id.clone())?;

        Ok(tab)
    }

    pub fn create_tab_in_session_background(
        &self,
        session_id: &str,
        url: String,
    ) -> Result<axiom_tabs::Tab> {
        self.session_manager.load_tabs_for_session(session_id)?;
        let previously_active = self.get_active_tab_in_session(session_id)?;

        let tab = self
            .session_manager
            .tab_manager()
            .create_tab(session_id.to_string(), url)?;
        let _ = self
            .session_manager
            .add_tab_to_session(session_id, tab.id.clone())?;

        if let Some(active) = previously_active {
            let _ = self.session_manager.tab_manager().blur_tab(&tab.id);
            let _ = self.session_manager.tab_manager().activate_tab(&active.id);
            return Ok(self.session_manager.tab_manager().get_tab(&tab.id)?);
        }

        Ok(tab)
    }

    pub fn activate_tab_in_session(
        &self,
        session_id: &str,
        tab_id: &str,
    ) -> Result<axiom_tabs::Tab> {
        self.session_manager.load_tabs_for_session(session_id)?;

        if let Some(active) = self.get_active_tab_in_session(session_id)? {
            if active.id != tab_id {
                let _ = self.session_manager.tab_manager().blur_tab(&active.id);
            }
        }

        Ok(self.session_manager.tab_manager().activate_tab(tab_id)?)
    }

    pub fn close_tab_in_session(&self, session_id: &str, tab_id: &str) -> Result<()> {
        self.session_manager.load_tabs_for_session(session_id)?;

        let tab = self.session_manager.tab_manager().get_tab(tab_id)?;
        let session = self.session_manager.get_session(session_id)?;
        let index = session
            .tab_order
            .iter()
            .position(|id| id == tab_id)
            .unwrap_or(session.tab_order.len());

        {
            let mut stack = self.recently_closed_tabs.write();
            stack.push(ClosedTab {
                session_id: tab.session_id.clone(),
                url: tab.url.clone(),
                title: tab.title.clone(),
                favicon_url: tab.favicon_url.clone(),
                index,
            });

            if stack.len() > 20 {
                let overflow = stack.len() - 20;
                stack.drain(0..overflow);
            }
        }

        let was_active = tab.state == axiom_tabs::TabState::Active;

        self.session_manager.tab_manager().close_tab(tab_id)?;
        let updated_session = self
            .session_manager
            .remove_tab_from_session(session_id, tab_id)?;

        if was_active && !updated_session.tab_order.is_empty() {
            let candidate_id = updated_session
                .tab_order
                .get(index.min(updated_session.tab_order.len().saturating_sub(1)))
                .cloned();

            if let Some(next_id) = candidate_id {
                let _ = self.session_manager.tab_manager().activate_tab(&next_id);
            }
        }

        Ok(())
    }

    pub fn reorder_tab_in_session(
        &self,
        session_id: &str,
        tab_id: &str,
        new_index: usize,
    ) -> Result<()> {
        let _ = self
            .session_manager
            .move_tab_in_session(session_id, tab_id, new_index)?;
        Ok(())
    }

    pub fn restore_last_closed_tab_in_session(&self, session_id: &str) -> Result<axiom_tabs::Tab> {
        let closed = {
            let mut stack = self.recently_closed_tabs.write();
            let idx = stack
                .iter()
                .rposition(|t| t.session_id == session_id)
                .ok_or_else(|| CoreError::Config("No recently closed tabs".to_string()))?;
            stack.remove(idx)
        };

        let tab = self.create_tab_in_session(session_id, closed.url)?;
        let _ = self
            .session_manager
            .move_tab_in_session(session_id, &tab.id, closed.index);

        if !closed.title.trim().is_empty() {
            let _ = self
                .session_manager
                .tab_manager()
                .set_tab_title(&tab.id, closed.title);
        }

        if closed.favicon_url.is_some() {
            let _ = self
                .session_manager
                .tab_manager()
                .set_tab_favicon(&tab.id, closed.favicon_url);
        }

        Ok(tab)
    }

    pub fn navigate_tab(&self, tab_id: &str, url: String) -> Result<axiom_tabs::Tab> {
        let tab = self
            .session_manager
            .tab_manager()
            .navigate_tab(tab_id, url.clone())?;

        // Record in history
        let _ = self.history_manager.record_visit(&url, "");

        Ok(tab)
    }

    pub fn update_tab_url_if_changed(&self, tab_id: &str, url: &str) -> Result<()> {
        let tab = self.session_manager.tab_manager().get_tab(tab_id)?;
        if tab.url == url {
            return Ok(());
        }

        let _ = self
            .session_manager
            .tab_manager()
            .navigate_tab(tab_id, url.to_string())?;

        let _ = self.history_manager.record_visit(url, "");

        Ok(())
    }

    pub fn restore_last_closed_tab(&self) -> Result<axiom_tabs::Tab> {
        let session = self.session_manager.active_session()?;

        let closed = {
            let mut stack = self.recently_closed_tabs.write();
            let idx = stack
                .iter()
                .rposition(|t| t.session_id == session.id)
                .ok_or_else(|| CoreError::Config("No recently closed tabs".to_string()))?;
            stack.remove(idx)
        };

        let tab = self.create_tab(closed.url)?;
        let _ = self.session_manager.move_tab(&tab.id, closed.index);

        if !closed.title.trim().is_empty() {
            let _ = self
                .session_manager
                .tab_manager()
                .set_tab_title(&tab.id, closed.title);
        }

        if closed.favicon_url.is_some() {
            let _ = self
                .session_manager
                .tab_manager()
                .set_tab_favicon(&tab.id, closed.favicon_url);
        }

        Ok(tab)
    }

    pub fn set_tab_title(&self, tab_id: &str, title: String) -> Result<axiom_tabs::Tab> {
        let tab = self
            .session_manager
            .tab_manager()
            .set_tab_title(tab_id, title.clone())?;

        if !tab.url.is_empty() && tab.url != "about:blank" {
            let _ = self.history_manager.update_title(&tab.url, &title);
        }

        Ok(tab)
    }

    pub fn reorder_tab(&self, tab_id: &str, new_index: usize) -> Result<()> {
        Ok(self.session_manager.move_tab(tab_id, new_index)?)
    }

    // === Navigation operations ===

    pub fn resolve_input(&self, input: &str) -> axiom_navigation::InputResolution {
        self.input_resolver.read().resolve(input)
    }

    pub fn search_history(&self, query: &str) -> Result<Vec<axiom_navigation::HistoryEntry>> {
        Ok(self.history_manager.search(query, 20)?)
    }

    pub fn recent_history(&self) -> Result<Vec<axiom_navigation::HistoryEntry>> {
        Ok(self.history_manager.recent(20)?)
    }

    pub fn clear_history_range(
        &self,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<()> {
        Ok(self.history_manager.clear_range(start, end)?)
    }

    // === Settings operations ===

    pub fn get_search_engine(&self) -> String {
        self.input_resolver.read().search_template().to_string()
    }

    pub fn set_search_engine(&self, template: String) -> Result<()> {
        self.input_resolver
            .write()
            .set_search_engine(template.clone());
        self.db.set_setting("search_engine", &template)?;
        Ok(())
    }

    pub fn get_theme(&self) -> Result<Option<String>> {
        Ok(self.db.get_setting("theme")?)
    }

    pub fn set_theme(&self, theme: String) -> Result<()> {
        self.db.set_setting("theme", &theme)?;
        Ok(())
    }

    pub fn get_bookmarks_bar_visible(&self) -> Result<bool> {
        Ok(self
            .db
            .get_setting("show_bookmarks_bar")?
            .map(|v| v == "true")
            .unwrap_or(true))
    }

    pub fn set_bookmarks_bar_visible(&self, visible: bool) -> Result<()> {
        self.db
            .set_setting("show_bookmarks_bar", if visible { "true" } else { "false" })?;
        Ok(())
    }

    pub fn get_autofill_enabled(&self) -> Result<bool> {
        Ok(self
            .db
            .get_setting("autofill_enabled")?
            .map(|v| v == "true")
            .unwrap_or(true))
    }

    pub fn set_autofill_enabled(&self, enabled: bool) -> Result<()> {
        self.db
            .set_setting("autofill_enabled", if enabled { "true" } else { "false" })?;
        Ok(())
    }

    pub fn get_autofill_name(&self) -> Result<Option<String>> {
        Ok(self.db.get_setting("autofill_name")?.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }))
    }

    pub fn get_autofill_email(&self) -> Result<Option<String>> {
        Ok(self.db.get_setting("autofill_email")?.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }))
    }

    pub fn set_autofill_profile(&self, name: Option<String>, email: Option<String>) -> Result<()> {
        let name = name.unwrap_or_default();
        let email = email.unwrap_or_default();
        self.db.set_setting("autofill_name", name.trim())?;
        self.db.set_setting("autofill_email", email.trim())?;
        Ok(())
    }

    pub fn get_password_save_prompt_enabled(&self) -> Result<bool> {
        Ok(self
            .db
            .get_setting("password_save_prompt_enabled")?
            .map(|v| v == "true")
            .unwrap_or(false))
    }

    pub fn set_password_save_prompt_enabled(&self, enabled: bool) -> Result<()> {
        self.db.set_setting(
            "password_save_prompt_enabled",
            if enabled { "true" } else { "false" },
        )?;
        Ok(())
    }

    pub fn get_bookmarks(&self) -> Result<Vec<Bookmark>> {
        match self.db.get_setting("bookmarks")? {
            Some(value) => Ok(serde_json::from_str(&value).unwrap_or_default()),
            None => Ok(Vec::new()),
        }
    }

    pub fn add_bookmark(
        &self,
        title: String,
        url: String,
        folder: Option<String>,
    ) -> Result<Vec<Bookmark>> {
        if url.trim().is_empty() {
            return Err(CoreError::Config(
                "Bookmark URL cannot be empty".to_string(),
            ));
        }

        let folder = crate::bookmarks::normalize_folder(folder);
        let mut bookmarks = self.get_bookmarks()?;
        if let Some(existing) = bookmarks.iter_mut().find(|b| b.url == url) {
            existing.title = title;
            if folder.is_some() {
                existing.folder = folder;
            }
        } else {
            bookmarks.push(Bookmark { title, url, folder });
        }

        let serialized = serde_json::to_string(&bookmarks)?;
        self.db.set_setting("bookmarks", &serialized)?;

        Ok(bookmarks)
    }

    pub fn remove_bookmark(&self, url: &str) -> Result<Vec<Bookmark>> {
        let mut bookmarks = self.get_bookmarks()?;
        bookmarks.retain(|bookmark| bookmark.url != url);

        let serialized = serde_json::to_string(&bookmarks)?;
        self.db.set_setting("bookmarks", &serialized)?;

        Ok(bookmarks)
    }

    pub fn update_bookmark(
        &self,
        old_url: &str,
        title: String,
        url: String,
        folder: Option<String>,
    ) -> Result<Vec<Bookmark>> {
        if url.trim().is_empty() {
            return Err(CoreError::Config(
                "Bookmark URL cannot be empty".to_string(),
            ));
        }

        let folder = crate::bookmarks::normalize_folder(folder);
        let mut bookmarks = self.get_bookmarks()?;

        let idx = bookmarks
            .iter()
            .position(|b| b.url == old_url)
            .ok_or_else(|| CoreError::Config("Bookmark not found".to_string()))?;

        if old_url != url && bookmarks.iter().any(|b| b.url == url) {
            return Err(CoreError::Config("Bookmark URL already exists".to_string()));
        }

        bookmarks[idx].title = title;
        bookmarks[idx].url = url;
        bookmarks[idx].folder = folder;

        let serialized = serde_json::to_string(&bookmarks)?;
        self.db.set_setting("bookmarks", &serialized)?;

        Ok(bookmarks)
    }

    pub fn get_bookmark_folders(&self) -> Result<Vec<String>> {
        Ok(crate::bookmarks::folders_from_bookmarks(
            &self.get_bookmarks()?,
        ))
    }

    pub fn export_bookmarks_html(&self) -> Result<String> {
        Ok(crate::bookmarks::export_bookmarks_html(
            &self.get_bookmarks()?,
        ))
    }

    pub fn import_bookmarks_html(&self, html: &str) -> Result<Vec<Bookmark>> {
        let mut bookmarks = self.get_bookmarks()?;
        let imported = crate::bookmarks::import_bookmarks_html(html);

        for bookmark in imported {
            if let Some(existing) = bookmarks.iter_mut().find(|b| b.url == bookmark.url) {
                existing.title = bookmark.title;
                if bookmark.folder.is_some() {
                    existing.folder = bookmark.folder;
                }
            } else {
                bookmarks.push(bookmark);
            }
        }

        let serialized = serde_json::to_string(&bookmarks)?;
        self.db.set_setting("bookmarks", &serialized)?;

        Ok(bookmarks)
    }

    // === Privacy operations ===

    pub fn check_permission(
        &self,
        origin: &str,
        permission_type: axiom_privacy::PermissionType,
    ) -> axiom_privacy::PermissionState {
        self.permission_manager
            .read()
            .get_permission(origin, permission_type)
    }

    pub fn set_permission(
        &self,
        origin: &str,
        permission_type: axiom_privacy::PermissionType,
        state: axiom_privacy::PermissionState,
    ) -> Result<()> {
        self.permission_manager
            .write()
            .set_site_permission(origin, permission_type, state);

        let serialized =
            serde_json::to_string(&self.permission_manager.read().export_permissions())?;
        self.db.set_setting("permissions", &serialized)?;
        Ok(())
    }

    pub fn should_block_url(&self, url: &str) -> bool {
        self.tracking_protection.read().should_block(url)
    }

    pub fn clean_url(&self, url: &str) -> String {
        self.tracking_protection.read().clean_url(url)
    }

    pub fn set_blocked_domains(&self, domains: Vec<String>) -> Result<usize> {
        let count = domains.len();
        let serialized = serde_json::to_string(&domains)?;
        self.db.set_setting("blocked_domains", &serialized)?;
        self.tracking_protection
            .write()
            .set_blocked_domains(domains);
        Ok(count)
    }

    pub fn blocked_domain_count(&self) -> usize {
        self.tracking_protection.read().blocked_domain_count()
    }

    // === Download operations ===

    pub fn download_manager(&self) -> &DownloadManager {
        &self.download_manager
    }

    pub fn create_download(
        &self,
        url: String,
        file_name: String,
    ) -> Result<axiom_download::Download> {
        Ok(self.download_manager.create_download(url, file_name)?)
    }

    // === Config ===

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn database(&self) -> &Database {
        &self.db
    }
}

impl Clone for Browser {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            db: self.db.clone(),
            session_manager: self.session_manager.clone(),
            history_manager: self.history_manager.clone(),
            input_resolver: Arc::clone(&self.input_resolver),
            download_manager: self.download_manager.clone(),
            permission_manager: Arc::clone(&self.permission_manager),
            tracking_protection: Arc::clone(&self.tracking_protection),
            active_tab_id: Arc::clone(&self.active_tab_id),
            recently_closed_tabs: Arc::clone(&self.recently_closed_tabs),
        }
    }
}

// Implement std::io::Error conversion for fs operations
impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Config(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            database_path: PathBuf::from(":memory:"),
            download_dir: PathBuf::from("/tmp/downloads"),
            search_engine: "https://duckduckgo.com/?q=%s".to_string(),
            homepage: "about:blank".to_string(),
            tracking_protection: true,
        }
    }

    #[test]
    fn test_browser_initialization() {
        // Use in-memory database for testing
        let db = Database::open_in_memory().unwrap();
        let config = test_config();

        let session_manager = SessionManager::new(db.clone());
        let history_manager = HistoryManager::new(db.clone());
        let input_resolver = Arc::new(RwLock::new(InputResolver::with_search_engine(
            config.search_engine.clone(),
        )));
        let download_manager = DownloadManager::new(db.clone(), config.download_dir.clone());

        let browser = Browser {
            config,
            db,
            session_manager,
            history_manager,
            input_resolver,
            download_manager,
            permission_manager: Arc::new(RwLock::new(PermissionManager::new())),
            tracking_protection: Arc::new(RwLock::new(TrackingProtection::new())),
            active_tab_id: Arc::new(RwLock::new(None)),
            recently_closed_tabs: Arc::new(RwLock::new(Vec::new())),
        };

        browser.session_manager.initialize().unwrap();

        // Create a tab
        let tab = browser
            .create_tab("https://example.com".to_string())
            .unwrap();
        assert_eq!(tab.url, "https://example.com");

        // Verify active tab
        let active = browser.get_active_tab().unwrap().unwrap();
        assert_eq!(active.id, tab.id);
    }
}

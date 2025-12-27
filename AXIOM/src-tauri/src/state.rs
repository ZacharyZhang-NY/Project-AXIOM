//! Application state management
use axiom_core::{Browser, Config, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Thread-safe application state wrapper
pub struct AppState {
    browser: Arc<RwLock<Option<Browser>>>,
    window_sessions: Arc<RwLock<HashMap<String, String>>>,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let config = Config::default();
        let browser = Browser::new(config)?;

        Ok(Self {
            browser: Arc::new(RwLock::new(Some(browser))),
            window_sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn initialize(&self) -> Result<()> {
        if let Some(browser) = self.browser.write().as_ref() {
            browser.initialize()?;
            if let Ok(active) = browser.session_manager().active_session() {
                self.window_sessions
                    .write()
                    .insert("main".to_string(), active.id);
            }
        }
        Ok(())
    }

    pub fn with_browser<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Browser) -> Result<T>,
    {
        let guard = self.browser.read();
        match guard.as_ref() {
            Some(browser) => f(browser),
            None => Err(axiom_core::CoreError::NotInitialized),
        }
    }

    pub fn session_id_for_window(&self, window_label: &str) -> Result<String> {
        if let Some(id) = self.window_sessions.read().get(window_label).cloned() {
            return Ok(id);
        }

        let id = self.with_browser(|browser| Ok(browser.session_manager().active_session()?.id))?;
        self.window_sessions
            .write()
            .insert(window_label.to_string(), id.clone());
        Ok(id)
    }

    pub fn set_session_for_window(&self, window_label: &str, session_id: String) {
        self.window_sessions
            .write()
            .insert(window_label.to_string(), session_id);
    }
}

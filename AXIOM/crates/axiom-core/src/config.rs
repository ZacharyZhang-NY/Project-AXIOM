//! Browser configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the database file
    pub database_path: PathBuf,
    /// Default download directory
    pub download_dir: PathBuf,
    /// Search engine URL template
    pub search_engine: String,
    /// Homepage URL
    pub homepage: String,
    /// Enable tracking protection
    pub tracking_protection: bool,
}

impl Config {
    pub fn new(data_dir: PathBuf) -> Self {
        let download_dir = dirs::download_dir().unwrap_or_else(|| data_dir.join("Downloads"));

        Self {
            database_path: data_dir.join("axiom.db"),
            download_dir,
            search_engine: "https://duckduckgo.com/?q=%s".to_string(),
            homepage: "about:blank".to_string(),
            tracking_protection: true,
        }
    }

    pub fn data_dir() -> PathBuf {
        dirs::data_local_dir()
            .map(|d| d.join("AXIOM"))
            .unwrap_or_else(|| PathBuf::from(".axiom"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new(Self::data_dir())
    }
}

// Simple dirs implementation for common directories
mod dirs {
    use std::path::PathBuf;

    pub fn data_local_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("LOCALAPPDATA").ok().map(PathBuf::from)
        }
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("Library/Application Support"))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_DATA_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".local/share"))
                })
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }

    pub fn download_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE")
                .ok()
                .map(|h| PathBuf::from(h).join("Downloads"))
        }
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("Downloads"))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_DOWNLOAD_DIR")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join("Downloads"))
                })
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }
}

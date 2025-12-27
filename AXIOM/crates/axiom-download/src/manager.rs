//! Download manager

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axiom_storage::Database;

use crate::download::{Download, DownloadState};
use crate::error::DownloadError;
use crate::Result;

pub struct DownloadManager {
    /// In-memory download cache
    downloads: Arc<RwLock<HashMap<String, Download>>>,
    /// Database for persistence
    db: Database,
    /// Default download directory
    download_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(db: Database, download_dir: PathBuf) -> Self {
        Self {
            downloads: Arc::new(RwLock::new(HashMap::new())),
            db,
            download_dir,
        }
    }

    /// Create a new download (pending user consent)
    pub fn create_download(&self, url: String, file_name: String) -> Result<Download> {
        let safe_file_name = sanitize_file_name(&file_name);
        let file_path = self.download_dir.join(&safe_file_name);
        let download = Download::new(url, file_path.to_string_lossy().to_string(), safe_file_name);

        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(download.id.clone(), download.clone());

        tracing::info!(
            download_id = %download.id,
            url = %download.url,
            "Created new download"
        );

        Ok(download)
    }

    /// Get a download by ID
    pub fn get_download(&self, id: &str) -> Result<Download> {
        self.downloads
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| DownloadError::NotFound(id.to_string()))
    }

    /// Start a download (after user consent)
    pub fn start_download(&self, id: &str) -> Result<Download> {
        let mut download = self.get_download(id)?;

        if download.state != DownloadState::Pending {
            return Err(DownloadError::Network(
                "Download is not in pending state".to_string(),
            ));
        }

        download.state = DownloadState::Downloading;
        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        tracing::info!(download_id = %id, "Started download");

        Ok(download)
    }

    /// Update download progress
    pub fn update_progress(
        &self,
        id: &str,
        downloaded: u64,
        total: Option<u64>,
    ) -> Result<Download> {
        let mut download = self.get_download(id)?;

        download.downloaded_bytes = downloaded;
        if let Some(t) = total {
            download.total_bytes = Some(t);
        }

        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        Ok(download)
    }

    pub fn set_mime_type(&self, id: &str, mime_type: Option<String>) -> Result<Download> {
        let mut download = self.get_download(id)?;
        download.mime_type = mime_type;

        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        Ok(download)
    }

    /// Pause a download
    pub fn pause_download(&self, id: &str) -> Result<Download> {
        let mut download = self.get_download(id)?;

        if download.state != DownloadState::Downloading {
            return Err(DownloadError::Network(
                "Download is not in progress".to_string(),
            ));
        }

        download.state = DownloadState::Paused;
        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        tracing::info!(download_id = %id, "Paused download");

        Ok(download)
    }

    /// Resume a download
    pub fn resume_download(&self, id: &str) -> Result<Download> {
        let mut download = self.get_download(id)?;

        if !download.can_resume() {
            return Err(DownloadError::Network(
                "Download cannot be resumed".to_string(),
            ));
        }

        download.state = DownloadState::Downloading;
        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        tracing::info!(download_id = %id, "Resumed download");

        Ok(download)
    }

    /// Complete a download
    pub fn complete_download(&self, id: &str, hash: Option<String>) -> Result<Download> {
        let mut download = self.get_download(id)?;

        download.state = DownloadState::Completed;
        download.hash = hash;
        download.completed_at = Some(chrono::Utc::now());

        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        tracing::info!(
            download_id = %id,
            hash = ?download.hash,
            "Completed download"
        );

        Ok(download)
    }

    /// Cancel a download
    pub fn cancel_download(&self, id: &str) -> Result<Download> {
        let mut download = self.get_download(id)?;

        download.state = DownloadState::Cancelled;
        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        tracing::info!(download_id = %id, "Cancelled download");

        Ok(download)
    }

    /// Mark download as failed
    pub fn fail_download(&self, id: &str, _reason: &str) -> Result<Download> {
        let mut download = self.get_download(id)?;

        download.state = DownloadState::Failed;
        self.save_download(&download)?;
        self.downloads
            .write()
            .insert(id.to_string(), download.clone());

        tracing::warn!(download_id = %id, "Download failed");

        Ok(download)
    }

    /// Get all downloads
    pub fn list_downloads(&self) -> Vec<Download> {
        self.downloads.read().values().cloned().collect()
    }

    /// Get active downloads
    pub fn active_downloads(&self) -> Vec<Download> {
        self.downloads
            .read()
            .values()
            .filter(|d| matches!(d.state, DownloadState::Downloading | DownloadState::Pending))
            .cloned()
            .collect()
    }

    /// Load downloads from database
    pub fn load_downloads(&self) -> Result<()> {
        use chrono::{DateTime, Utc};

        let downloads = self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, url, file_path, file_name, mime_type, total_bytes,
                        downloaded_bytes, state, hash, created_at, completed_at
                 FROM downloads",
            )?;

            let downloads: Vec<Download> = stmt
                .query_map([], |row| {
                    let state_str: String = row.get(7)?;
                    let state: DownloadState = state_str.parse().unwrap_or(DownloadState::Pending);

                    // Parse datetime strings
                    let created_str: String = row.get(9)?;
                    let completed_str: Option<String> = row.get(10)?;

                    let created_at = DateTime::parse_from_rfc3339(&created_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());
                    let completed_at = completed_str.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .map(|dt| dt.with_timezone(&Utc))
                            .ok()
                    });

                    Ok(Download {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        file_path: row.get(2)?,
                        file_name: row.get(3)?,
                        mime_type: row.get(4)?,
                        total_bytes: row.get(5)?,
                        downloaded_bytes: row.get::<_, i64>(6)? as u64,
                        state,
                        hash: row.get(8)?,
                        created_at,
                        completed_at,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(downloads)
        })?;

        let mut cache = self.downloads.write();
        for download in downloads {
            cache.insert(download.id.clone(), download);
        }

        Ok(())
    }

    /// Save download to database
    fn save_download(&self, download: &Download) -> Result<()> {
        Ok(self.db.with_connection(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO downloads
                 (id, url, file_path, file_name, mime_type, total_bytes,
                  downloaded_bytes, state, hash, created_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    download.id,
                    download.url,
                    download.file_path,
                    download.file_name,
                    download.mime_type,
                    download.total_bytes.map(|v| v as i64),
                    download.downloaded_bytes as i64,
                    download.state.as_str(),
                    download.hash,
                    download.created_at.to_rfc3339(),
                    download.completed_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
            Ok(())
        })?)
    }
}

impl Clone for DownloadManager {
    fn clone(&self) -> Self {
        Self {
            downloads: Arc::clone(&self.downloads),
            db: self.db.clone(),
            download_dir: self.download_dir.clone(),
        }
    }
}

fn sanitize_file_name(file_name: &str) -> String {
    let name = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download")
        .trim();

    if name.is_empty() {
        "download".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_manager() {
        let db = Database::open_in_memory().unwrap();
        let manager = DownloadManager::new(db, PathBuf::from("/downloads"));

        // Create a download
        let download = manager
            .create_download(
                "https://example.com/file.pdf".to_string(),
                "file.pdf".to_string(),
            )
            .unwrap();

        assert_eq!(download.state, DownloadState::Pending);

        // Start download
        let started = manager.start_download(&download.id).unwrap();
        assert_eq!(started.state, DownloadState::Downloading);

        // Update progress
        manager
            .update_progress(&download.id, 500, Some(1000))
            .unwrap();
        let updated = manager.get_download(&download.id).unwrap();
        assert_eq!(updated.downloaded_bytes, 500);

        // Complete download
        let completed = manager
            .complete_download(&download.id, Some("abc123".to_string()))
            .unwrap();
        assert_eq!(completed.state, DownloadState::Completed);
        assert_eq!(completed.hash, Some("abc123".to_string()));
    }
}

//! Download data structure

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadState {
    /// Waiting for user consent
    Pending,
    /// Download in progress
    Downloading,
    /// Download paused (for resume)
    Paused,
    /// Download completed successfully
    Completed,
    /// Download failed
    Failed,
    /// Download cancelled by user
    Cancelled,
}

impl DownloadState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadState::Pending => "pending",
            DownloadState::Downloading => "downloading",
            DownloadState::Paused => "paused",
            DownloadState::Completed => "completed",
            DownloadState::Failed => "failed",
            DownloadState::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for DownloadState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(DownloadState::Pending),
            "downloading" => Ok(DownloadState::Downloading),
            "paused" => Ok(DownloadState::Paused),
            "completed" => Ok(DownloadState::Completed),
            "failed" => Ok(DownloadState::Failed),
            "cancelled" => Ok(DownloadState::Cancelled),
            _ => Err(format!("Unknown download state: {}", s)),
        }
    }
}

/// Risk level based on MIME type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Warning,
    Dangerous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub id: String,
    pub url: String,
    pub file_path: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub state: DownloadState,
    pub hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Download {
    pub fn new(url: String, file_path: String, file_name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            url,
            file_path,
            file_name,
            mime_type: None,
            total_bytes: None,
            downloaded_bytes: 0,
            state: DownloadState::Pending,
            hash: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Get download progress as percentage (0-100)
    pub fn progress(&self) -> f64 {
        match self.total_bytes {
            Some(total) if total > 0 => {
                (self.downloaded_bytes as f64 / total as f64 * 100.0).min(100.0)
            }
            _ => 0.0,
        }
    }

    /// Check if download can be resumed
    pub fn can_resume(&self) -> bool {
        matches!(self.state, DownloadState::Paused | DownloadState::Failed)
            && self.downloaded_bytes > 0
    }

    /// Get risk level based on MIME type
    pub fn risk_level(&self) -> RiskLevel {
        match self.mime_type.as_deref() {
            // Executables are dangerous
            Some(mime) if mime.contains("executable") => RiskLevel::Dangerous,
            Some(mime) if mime.contains("x-msdownload") => RiskLevel::Dangerous,
            Some(mime) if mime.contains("x-msdos-program") => RiskLevel::Dangerous,

            // Scripts need warning
            Some(mime) if mime.contains("javascript") => RiskLevel::Warning,
            Some(mime) if mime.contains("x-sh") => RiskLevel::Warning,
            Some(mime) if mime.contains("x-python") => RiskLevel::Warning,

            // Archives need warning
            Some(mime) if mime.contains("zip") => RiskLevel::Warning,
            Some(mime) if mime.contains("x-rar") => RiskLevel::Warning,
            Some(mime) if mime.contains("x-7z") => RiskLevel::Warning,
            Some(mime) if mime.contains("x-tar") => RiskLevel::Warning,

            // Safe types
            Some(mime) if mime.starts_with("image/") => RiskLevel::Safe,
            Some(mime) if mime.starts_with("audio/") => RiskLevel::Safe,
            Some(mime) if mime.starts_with("video/") => RiskLevel::Safe,
            Some(mime) if mime.starts_with("text/") => RiskLevel::Safe,
            Some("application/pdf") => RiskLevel::Safe,

            // Unknown defaults to warning
            _ => RiskLevel::Warning,
        }
    }

    /// Check if this is a risky download that needs user warning
    pub fn needs_warning(&self) -> bool {
        self.risk_level() != RiskLevel::Safe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_download() {
        let download = Download::new(
            "https://example.com/file.pdf".to_string(),
            "/downloads/file.pdf".to_string(),
            "file.pdf".to_string(),
        );

        assert_eq!(download.state, DownloadState::Pending);
        assert_eq!(download.downloaded_bytes, 0);
        assert!(download.completed_at.is_none());
    }

    #[test]
    fn test_progress() {
        let mut download = Download::new(
            "https://example.com/file.zip".to_string(),
            "/downloads/file.zip".to_string(),
            "file.zip".to_string(),
        );

        download.total_bytes = Some(1000);
        download.downloaded_bytes = 500;

        assert!((download.progress() - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_risk_level() {
        let mut download = Download::new(
            "https://example.com/file".to_string(),
            "/downloads/file".to_string(),
            "file".to_string(),
        );

        download.mime_type = Some("image/png".to_string());
        assert_eq!(download.risk_level(), RiskLevel::Safe);

        download.mime_type = Some("application/x-msdownload".to_string());
        assert_eq!(download.risk_level(), RiskLevel::Dangerous);

        download.mime_type = Some("application/zip".to_string());
        assert_eq!(download.risk_level(), RiskLevel::Warning);
    }
}

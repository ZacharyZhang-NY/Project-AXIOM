//! Navigation and address bar commands

use chrono::{DateTime, FixedOffset, Utc};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

use super::tabs::CommandResult;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum InputResolutionResult {
    Navigate(String),
    Search(String),
    Command {
        command_type: String,
        query: Option<String>,
    },
}

impl From<axiom_core::InputResolution> for InputResolutionResult {
    fn from(resolution: axiom_core::InputResolution) -> Self {
        match resolution {
            axiom_core::InputResolution::Navigate(url) => InputResolutionResult::Navigate(url),
            axiom_core::InputResolution::Search(url) => InputResolutionResult::Search(url),
            axiom_core::InputResolution::Command(cmd) => InputResolutionResult::Command {
                command_type: match cmd.command_type {
                    axiom_core::CommandType::Tabs => "tabs".to_string(),
                    axiom_core::CommandType::History => "history".to_string(),
                    axiom_core::CommandType::Sessions => "sessions".to_string(),
                },
                query: cmd.query,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntryInfo {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: String,
    pub visit_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProbeInfo {
    pub ok: bool,
    pub status: Option<u16>,
    pub final_url: Option<String>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
}

impl From<axiom_core::HistoryEntry> for HistoryEntryInfo {
    fn from(entry: axiom_core::HistoryEntry) -> Self {
        Self {
            id: entry.id,
            url: entry.url,
            title: entry.title,
            visited_at: entry.visited_at.to_rfc3339(),
            visit_count: entry.visit_count,
        }
    }
}

#[tauri::command]
pub fn resolve_input(
    state: State<AppState>,
    input: String,
) -> CommandResult<InputResolutionResult> {
    match state.with_browser(|browser| Ok(browser.resolve_input(&input))) {
        Ok(resolution) => CommandResult::ok(resolution.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn search_history(
    state: State<AppState>,
    query: String,
) -> CommandResult<Vec<HistoryEntryInfo>> {
    match state.with_browser(|browser| browser.search_history(&query)) {
        Ok(entries) => CommandResult::ok(entries.into_iter().map(HistoryEntryInfo::from).collect()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_recent_history(state: State<AppState>) -> CommandResult<Vec<HistoryEntryInfo>> {
    match state.with_browser(|browser| browser.recent_history()) {
        Ok(entries) => CommandResult::ok(entries.into_iter().map(HistoryEntryInfo::from).collect()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn clear_history_range(
    state: State<AppState>,
    start: Option<String>,
    end: Option<String>,
) -> CommandResult<()> {
    let start = start
        .as_deref()
        .and_then(|s| DateTime::<FixedOffset>::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let end = end
        .as_deref()
        .and_then(|s| DateTime::<FixedOffset>::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    match state.with_browser(|browser| browser.clear_history_range(start, end)) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub async fn probe_url(url: String) -> CommandResult<ProbeInfo> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return CommandResult::ok(ProbeInfo {
            ok: false,
            status: None,
            final_url: None,
            error_kind: Some("invalid_url".to_string()),
            error_message: Some("URL is empty".to_string()),
        });
    }

    if trimmed.starts_with("about:") || trimmed.starts_with("axiom:") {
        return CommandResult::ok(ProbeInfo {
            ok: true,
            status: None,
            final_url: Some(trimmed.to_string()),
            error_kind: None,
            error_message: None,
        });
    }

    let parsed = match url::Url::parse(trimmed) {
        Ok(u) => u,
        Err(e) => {
            return CommandResult::ok(ProbeInfo {
                ok: false,
                status: None,
                final_url: None,
                error_kind: Some("invalid_url".to_string()),
                error_message: Some(e.to_string()),
            });
        }
    };

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return CommandResult::ok(ProbeInfo {
            ok: true,
            status: None,
            final_url: Some(trimmed.to_string()),
            error_kind: None,
            error_message: None,
        });
    }

    let client = match reqwest::Client::builder()
        .redirect(Policy::limited(5))
        .timeout(Duration::from_secs(6))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return CommandResult::ok(ProbeInfo {
                ok: false,
                status: None,
                final_url: None,
                error_kind: Some("client".to_string()),
                error_message: Some(e.to_string()),
            });
        }
    };

    let response = match client.head(parsed.clone()).send().await {
        Ok(resp) => Ok(resp),
        Err(_) => {
            client
                .get(parsed)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await
        }
    };

    match response {
        Ok(resp) => CommandResult::ok(ProbeInfo {
            ok: true,
            status: Some(resp.status().as_u16()),
            final_url: Some(resp.url().to_string()),
            error_kind: None,
            error_message: None,
        }),
        Err(e) => {
            let msg = e.to_string();
            let mut kind = if e.is_timeout() {
                "timeout"
            } else if e.is_connect() {
                "connect"
            } else if e.is_request() {
                "request"
            } else {
                "unknown"
            }
            .to_string();

            let lowered = msg.to_lowercase();
            if kind == "connect" {
                if lowered.contains("dns")
                    || lowered.contains("resolve")
                    || lowered.contains("name")
                {
                    kind = "dns".to_string();
                } else if lowered.contains("tls")
                    || lowered.contains("certificate")
                    || lowered.contains("handshake")
                {
                    kind = "tls".to_string();
                }
            }

            CommandResult::ok(ProbeInfo {
                ok: false,
                status: None,
                final_url: None,
                error_kind: Some(kind),
                error_message: Some(msg),
            })
        }
    }
}

//! Privacy and permission commands

use chrono::{DateTime, FixedOffset, Utc};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;
use tauri::State;

use super::tabs::CommandResult;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionTypeArg {
    Camera,
    Microphone,
    Location,
    Notifications,
    WebRTC,
}

impl From<PermissionTypeArg> for axiom_core::PermissionType {
    fn from(arg: PermissionTypeArg) -> Self {
        match arg {
            PermissionTypeArg::Camera => axiom_core::PermissionType::Camera,
            PermissionTypeArg::Microphone => axiom_core::PermissionType::Microphone,
            PermissionTypeArg::Location => axiom_core::PermissionType::Location,
            PermissionTypeArg::Notifications => axiom_core::PermissionType::Notifications,
            PermissionTypeArg::WebRTC => axiom_core::PermissionType::WebRTC,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionStateArg {
    Ask,
    Allow,
    Deny,
}

impl From<PermissionStateArg> for axiom_core::PermissionState {
    fn from(arg: PermissionStateArg) -> Self {
        match arg {
            PermissionStateArg::Ask => axiom_core::PermissionState::Ask,
            PermissionStateArg::Allow => axiom_core::PermissionState::Allow,
            PermissionStateArg::Deny => axiom_core::PermissionState::Deny,
        }
    }
}

impl From<axiom_core::PermissionState> for PermissionStateArg {
    fn from(state: axiom_core::PermissionState) -> Self {
        match state {
            axiom_core::PermissionState::Ask => PermissionStateArg::Ask,
            axiom_core::PermissionState::Allow => PermissionStateArg::Allow,
            axiom_core::PermissionState::Deny => PermissionStateArg::Deny,
        }
    }
}

#[tauri::command]
pub fn check_permission(
    state: State<AppState>,
    origin: String,
    permission_type: PermissionTypeArg,
) -> CommandResult<PermissionStateArg> {
    match state
        .with_browser(|browser| Ok(browser.check_permission(&origin, permission_type.into())))
    {
        Ok(permission_state) => CommandResult::ok(permission_state.into()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn set_permission(
    state: State<AppState>,
    origin: String,
    permission_type: PermissionTypeArg,
    permission_state: PermissionStateArg,
) -> CommandResult<()> {
    match state.with_browser(|browser| {
        browser.set_permission(&origin, permission_type.into(), permission_state.into())?;
        Ok(())
    }) {
        Ok(()) => CommandResult::ok(()),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn should_block_url(state: State<AppState>, url: String) -> CommandResult<bool> {
    match state.with_browser(|browser| Ok(browser.should_block_url(&url))) {
        Ok(should_block) => CommandResult::ok(should_block),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[tauri::command]
pub fn clean_url(state: State<AppState>, url: String) -> CommandResult<String> {
    match state.with_browser(|browser| Ok(browser.clean_url(&url))) {
        Ok(cleaned) => CommandResult::ok(cleaned),
        Err(e) => CommandResult::err(e.to_string()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterListsStatus {
    pub blocked_domains: usize,
    pub updated: bool,
}

#[tauri::command]
pub async fn refresh_filter_lists(
    state: State<'_, AppState>,
    force: Option<bool>,
) -> Result<FilterListsStatus, String> {
    let force = force.unwrap_or(false);

    if !force {
        let last = state.with_browser(|browser| {
            Ok(browser
                .database()
                .get_setting("blocked_domains_last_updated")?)
        });
        if let Ok(Some(value)) = last {
            let parsed = DateTime::<FixedOffset>::parse_from_rfc3339(&value)
                .ok()
                .map(|dt| dt.with_timezone(&Utc));
            if let Some(last_dt) = parsed {
                if (Utc::now() - last_dt).num_days() < 7 {
                    let count = state.with_browser(|browser| Ok(browser.blocked_domain_count()));
                    return Ok(FilterListsStatus {
                        blocked_domains: count.unwrap_or(0),
                        updated: false,
                    });
                }
            }
        }
    }

    let client = match reqwest::Client::builder()
        .redirect(Policy::limited(3))
        .timeout(Duration::from_secs(20))
        .user_agent("Mozilla/5.0 (AXIOM)")
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(e.to_string()),
    };

    let easylist = client
        .get("https://easylist.to/easylist/easylist.txt")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let easyprivacy = client
        .get("https://easylist.to/easylist/easyprivacy.txt")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let mut domains: HashSet<String> = HashSet::new();
    parse_abp_domains(&easylist, &mut domains);
    parse_abp_domains(&easyprivacy, &mut domains);

    let mut domains: Vec<String> = domains.into_iter().collect();
    domains.sort();

    let count = state
        .with_browser(|browser| browser.set_blocked_domains(domains))
        .map_err(|e| e.to_string())?;

    let _ = state.with_browser(|browser| {
        browser
            .database()
            .set_setting("blocked_domains_last_updated", &Utc::now().to_rfc3339())?;
        Ok(())
    });

    Ok(FilterListsStatus {
        blocked_domains: count,
        updated: true,
    })
}

fn parse_abp_domains(list: &str, out: &mut HashSet<String>) {
    for raw in list.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('!') || line.starts_with('[') {
            continue;
        }
        if line.starts_with("@@") {
            continue;
        }

        let Some(rest) = line.strip_prefix("||") else {
            continue;
        };

        let mut end = rest.len();
        for (idx, ch) in rest.char_indices() {
            if ch == '^' || ch == '/' || ch == '$' {
                end = idx;
                break;
            }
        }

        let domain = rest[..end].trim_matches('.');
        if domain.is_empty() {
            continue;
        }
        if domain.contains('*') || domain.contains('|') || domain.contains('%') {
            continue;
        }
        if !domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_')
        {
            continue;
        }
        if !domain.contains('.') {
            continue;
        }

        out.insert(domain.to_lowercase());
    }
}

//! Permission management
//!
//! Per PRD Section 5.4:
//! | Capability    | Default  | Persistence |
//! | Camera        | Ask      | Per-site    |
//! | Mic           | Ask      | Per-site    |
//! | Location      | Ask      | Per-site    |
//! | Notifications | Deny     | Manual      |
//! | WebRTC        | Disabled | Global      |

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionType {
    Camera,
    Microphone,
    Location,
    Notifications,
    WebRTC,
}

impl PermissionType {
    /// Get the default state for this permission type (per PRD)
    pub fn default_state(&self) -> PermissionState {
        match self {
            PermissionType::Camera => PermissionState::Ask,
            PermissionType::Microphone => PermissionState::Ask,
            PermissionType::Location => PermissionState::Ask,
            PermissionType::Notifications => PermissionState::Deny,
            PermissionType::WebRTC => PermissionState::Deny, // Disabled globally
        }
    }

    /// Whether this permission is per-site or global
    pub fn is_per_site(&self) -> bool {
        match self {
            PermissionType::Camera => true,
            PermissionType::Microphone => true,
            PermissionType::Location => true,
            PermissionType::Notifications => false, // Manual (global)
            PermissionType::WebRTC => false,        // Global
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionState {
    /// Prompt user when requested
    Ask,
    /// Always allow
    Allow,
    /// Always deny
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub permission_type: PermissionType,
    pub state: PermissionState,
    pub origin: Option<String>, // None for global permissions
}

pub struct PermissionManager {
    /// Site-specific permissions: (origin, type) -> state
    site_permissions: HashMap<(String, PermissionType), PermissionState>,
    /// Global permissions
    global_permissions: HashMap<PermissionType, PermissionState>,
}

impl PermissionManager {
    pub fn new() -> Self {
        let mut global = HashMap::new();

        // Set defaults per PRD
        global.insert(PermissionType::Notifications, PermissionState::Deny);
        global.insert(PermissionType::WebRTC, PermissionState::Deny);

        Self {
            site_permissions: HashMap::new(),
            global_permissions: global,
        }
    }

    /// Get permission state for a specific origin and type
    pub fn get_permission(&self, origin: &str, permission_type: PermissionType) -> PermissionState {
        // Check if it's a global permission
        if !permission_type.is_per_site() {
            return self
                .global_permissions
                .get(&permission_type)
                .copied()
                .unwrap_or_else(|| permission_type.default_state());
        }

        // Check site-specific permission
        self.site_permissions
            .get(&(origin.to_string(), permission_type))
            .copied()
            .unwrap_or_else(|| permission_type.default_state())
    }

    /// Set permission for a specific origin
    pub fn set_site_permission(
        &mut self,
        origin: &str,
        permission_type: PermissionType,
        state: PermissionState,
    ) {
        if permission_type.is_per_site() {
            self.site_permissions
                .insert((origin.to_string(), permission_type), state);
        }
    }

    /// Set global permission
    pub fn set_global_permission(
        &mut self,
        permission_type: PermissionType,
        state: PermissionState,
    ) {
        if !permission_type.is_per_site() {
            self.global_permissions.insert(permission_type, state);
        }
    }

    /// Clear permission for a specific origin
    pub fn clear_site_permission(&mut self, origin: &str, permission_type: PermissionType) {
        self.site_permissions
            .remove(&(origin.to_string(), permission_type));
    }

    /// Get all permissions for an origin
    pub fn get_site_permissions(&self, origin: &str) -> Vec<Permission> {
        let mut permissions = Vec::new();

        for permission_type in [
            PermissionType::Camera,
            PermissionType::Microphone,
            PermissionType::Location,
        ] {
            let state = self.get_permission(origin, permission_type);
            permissions.push(Permission {
                permission_type,
                state,
                origin: Some(origin.to_string()),
            });
        }

        permissions
    }

    /// Check if a permission should prompt the user
    pub fn should_prompt(&self, origin: &str, permission_type: PermissionType) -> bool {
        self.get_permission(origin, permission_type) == PermissionState::Ask
    }

    /// Check if a permission is allowed
    pub fn is_allowed(&self, origin: &str, permission_type: PermissionType) -> bool {
        self.get_permission(origin, permission_type) == PermissionState::Allow
    }

    pub fn export_permissions(&self) -> Vec<Permission> {
        let mut out = Vec::new();

        for ((origin, permission_type), state) in &self.site_permissions {
            out.push(Permission {
                permission_type: *permission_type,
                state: *state,
                origin: Some(origin.clone()),
            });
        }

        for (permission_type, state) in &self.global_permissions {
            out.push(Permission {
                permission_type: *permission_type,
                state: *state,
                origin: None,
            });
        }

        out.sort_by(|a, b| match (&a.origin, &b.origin) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        out
    }

    pub fn import_permissions(&mut self, permissions: Vec<Permission>) {
        *self = PermissionManager::new();

        for perm in permissions {
            match perm.origin {
                Some(origin) => {
                    self.set_site_permission(&origin, perm.permission_type, perm.state);
                }
                None => {
                    self.set_global_permission(perm.permission_type, perm.state);
                }
            }
        }
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_permissions() {
        let manager = PermissionManager::new();

        // Camera should prompt by default
        assert_eq!(
            manager.get_permission("https://example.com", PermissionType::Camera),
            PermissionState::Ask
        );

        // Notifications should be denied by default
        assert_eq!(
            manager.get_permission("https://example.com", PermissionType::Notifications),
            PermissionState::Deny
        );

        // WebRTC should be disabled
        assert_eq!(
            manager.get_permission("https://example.com", PermissionType::WebRTC),
            PermissionState::Deny
        );
    }

    #[test]
    fn test_site_permission() {
        let mut manager = PermissionManager::new();

        // Allow camera for specific site
        manager.set_site_permission(
            "https://meet.google.com",
            PermissionType::Camera,
            PermissionState::Allow,
        );

        assert!(manager.is_allowed("https://meet.google.com", PermissionType::Camera));
        assert!(manager.should_prompt("https://other.com", PermissionType::Camera));
    }
}

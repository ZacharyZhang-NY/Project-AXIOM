//! AXIOM Privacy Protection
//!
//! Per PRD Section 5.4:
//! Default-On Protections:
//! - EasyList + EasyPrivacy
//! - Block third-party cookies
//! - Strip known tracking parameters
//! - Disable background popups
//!
//! Permissions Model:
//! - Camera: Ask (Per-site)
//! - Mic: Ask (Per-site)
//! - Location: Ask (Per-site)
//! - Notifications: Deny (Manual)
//! - WebRTC: Disabled (Global)

mod permissions;
mod tracking;

pub use permissions::{Permission, PermissionManager, PermissionState, PermissionType};
pub use tracking::{TrackingProtection, TrackingRule};

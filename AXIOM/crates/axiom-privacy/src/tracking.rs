//! Tracking protection
//!
//! Implements URL-based blocking and parameter stripping

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

/// Known tracking parameters to strip from URLs
const TRACKING_PARAMS: &[&str] = &[
    // Google Analytics
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "utm_cid",
    // Facebook
    "fbclid",
    "fb_action_ids",
    "fb_action_types",
    "fb_source",
    "fb_ref",
    // Twitter
    "twclid",
    // Microsoft
    "msclkid",
    // Google
    "gclid",
    "gclsrc",
    "dclid",
    // Generic
    "ref",
    "ref_",
    "referrer",
    "_ga",
    "_gl",
    // Others
    "mc_eid",
    "mc_cid",
    "oly_anon_id",
    "oly_enc_id",
    "_openstat",
    "vero_id",
    "wickedid",
    "yclid",
    "igshid",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackingRule {
    pub pattern: String,
    pub action: TrackingAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackingAction {
    Block,
    Allow,
}

pub struct TrackingProtection {
    /// Blocked domains
    blocked_domains: HashSet<String>,
    /// Domains we never block (search engines, common CDNs)
    allow_domains: HashSet<String>,
    /// Tracking parameters to strip
    strip_params: HashSet<String>,
    /// Whether protection is enabled
    enabled: bool,
}

impl TrackingProtection {
    pub fn new() -> Self {
        let mut strip_params = HashSet::new();
        for param in TRACKING_PARAMS {
            strip_params.insert(param.to_string());
        }

        let allow_domains: HashSet<String> = [
            // Search engines
            "google.com",
            "www.google.com",
            "bing.com",
            "www.bing.com",
            "duckduckgo.com",
            "www.duckduckgo.com",
            // Video / media
            "youtube.com",
            "www.youtube.com",
            "youtu.be",
            "googlevideo.com",
            "gstatic.com",
            "ytimg.com",
            // Known safe
            "stablelance.com",
            "www.stablelance.com",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            blocked_domains: HashSet::new(),
            allow_domains,
            strip_params,
            enabled: true,
        }
    }

    /// Enable or disable protection
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if protection is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Add a domain to block list
    pub fn block_domain(&mut self, domain: &str) {
        self.blocked_domains.insert(domain.to_lowercase());
    }

    pub fn set_blocked_domains<I>(&mut self, domains: I)
    where
        I: IntoIterator<Item = String>,
    {
        self.blocked_domains = domains.into_iter().map(|d| d.to_lowercase()).collect();
    }

    pub fn blocked_domain_count(&self) -> usize {
        self.blocked_domains.len()
    }

    /// Check if a URL should be blocked
    pub fn should_block(&self, url: &str) -> bool {
        if !self.enabled {
            return false;
        }

        if self.blocked_domains.is_empty() {
            return false;
        }

        if let Ok(parsed) = Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                let host = host.to_lowercase();

                // Never block allowlisted domains or their parent domains
                let parts: Vec<&str> = host.split('.').collect();
                for i in 0..parts.len() {
                    let parent = parts[i..].join(".");
                    if self.allow_domains.contains(&parent) {
                        return false;
                    }
                }

                // Check exact match
                if self.blocked_domains.contains(&host) {
                    return true;
                }

                // Check parent domains
                for i in 0..parts.len() {
                    let parent = parts[i..].join(".");
                    if self.blocked_domains.contains(&parent) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Strip tracking parameters from URL
    pub fn clean_url(&self, url: &str) -> String {
        if !self.enabled {
            return url.to_string();
        }

        match Url::parse(url) {
            Ok(mut parsed) => {
                let pairs: Vec<(String, String)> = parsed
                    .query_pairs()
                    .filter(|(key, _)| !self.strip_params.contains(key.as_ref()))
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect();

                if pairs.is_empty() {
                    parsed.set_query(None);
                } else {
                    let query: String = pairs
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("&");
                    parsed.set_query(Some(&query));
                }

                parsed.to_string()
            }
            Err(_) => url.to_string(),
        }
    }

    /// Check if a request is third-party
    pub fn is_third_party(page_url: &str, request_url: &str) -> bool {
        let page = match Url::parse(page_url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let request = match Url::parse(request_url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        let page_host = page.host_str().unwrap_or("");
        let request_host = request.host_str().unwrap_or("");

        // Extract registrable domain (simplified)
        fn get_base_domain(host: &str) -> &str {
            let parts: Vec<&str> = host.split('.').collect();
            if parts.len() >= 2 {
                let len = parts.len();
                // Handle cases like co.uk, com.au (simplified)
                if parts[len - 1].len() <= 2 && parts.len() >= 3 {
                    return &host[host.len() - parts[len - 3..].join(".").len()..];
                }
                return &host[host.len() - parts[len - 2..].join(".").len()..];
            }
            host
        }

        get_base_domain(page_host) != get_base_domain(request_host)
    }
}

impl Default for TrackingProtection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_url() {
        let protection = TrackingProtection::new();

        let cleaned = protection
            .clean_url("https://example.com/page?id=123&utm_source=test&utm_campaign=demo");
        assert_eq!(cleaned, "https://example.com/page?id=123");

        let cleaned = protection.clean_url("https://example.com/page?fbclid=123");
        assert_eq!(cleaned, "https://example.com/page");
    }

    #[test]
    fn test_block_domain() {
        let mut protection = TrackingProtection::new();
        protection.block_domain("tracker.com");

        assert!(protection.should_block("https://tracker.com/pixel.gif"));
        assert!(protection.should_block("https://sub.tracker.com/script.js"));
        assert!(!protection.should_block("https://example.com/page"));
    }

    #[test]
    fn test_third_party() {
        assert!(TrackingProtection::is_third_party(
            "https://example.com",
            "https://cdn.other.com/script.js"
        ));

        assert!(!TrackingProtection::is_third_party(
            "https://example.com",
            "https://cdn.example.com/script.js"
        ));
    }
}

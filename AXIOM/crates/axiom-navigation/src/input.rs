//! Input resolution for address bar
//!
//! Per PRD Section 5.3:
//! 1. Valid URL → navigate
//! 2. Invalid URL → search
//! 3. `@command` → internal command mode

use std::net::IpAddr;
use url::Url;

use crate::command::Command;

/// Result of resolving address bar input
#[derive(Debug, Clone)]
pub enum InputResolution {
    /// Navigate to a URL
    Navigate(String),
    /// Perform a search
    Search(String),
    /// Execute a command
    Command(Command),
}

pub struct InputResolver {
    /// Search engine URL template (%s replaced with query)
    search_template: String,
}

impl InputResolver {
    pub fn new() -> Self {
        Self {
            // Default to DuckDuckGo (privacy-focused per PRD philosophy)
            search_template: "https://duckduckgo.com/?q=%s".to_string(),
        }
    }

    pub fn with_search_engine(template: String) -> Self {
        Self {
            search_template: template,
        }
    }

    pub fn set_search_engine(&mut self, template: String) {
        self.search_template = template;
    }

    pub fn search_template(&self) -> &str {
        &self.search_template
    }

    /// Resolve user input into an action
    pub fn resolve(&self, input: &str) -> InputResolution {
        let input = input.trim();

        if input.is_empty() {
            return InputResolution::Navigate("about:blank".to_string());
        }

        // Check for command mode first
        if input.starts_with('@') {
            if let Some(command) = Command::parse(input) {
                return InputResolution::Command(command);
            }
        }

        // Try to parse as URL
        if let Some(url) = self.try_parse_url(input) {
            return InputResolution::Navigate(url);
        }

        // Fall back to search
        let search_url = self.build_search_url(input);
        InputResolution::Search(search_url)
    }

    /// Try to parse input as a valid URL
    fn try_parse_url(&self, input: &str) -> Option<String> {
        // Direct URL with scheme
        if (input.starts_with("http://") || input.starts_with("https://"))
            && Url::parse(input).is_ok()
        {
            return Some(input.to_string());
        }

        // URL without scheme - check if it looks like a domain
        if self.looks_like_url(input) {
            let (host, rest) = Self::split_host_and_rest(input);
            let with_https = if self.is_ipv6_host(host) && !host.starts_with('[') {
                format!("https://[{}]{}", host, rest)
            } else {
                format!("https://{}{}", host, rest)
            };

            if Url::parse(&with_https).is_ok() {
                return Some(with_https);
            }
        }

        // Special protocols
        if input.starts_with("file://") || input.starts_with("about:") || input.starts_with("data:")
        {
            return Some(input.to_string());
        }

        None
    }

    /// Heuristic check if input looks like a URL
    fn looks_like_url(&self, input: &str) -> bool {
        // Contains a dot and no spaces
        if input.contains(' ') {
            return false;
        }

        // localhost or IP address
        if input.starts_with("localhost") || self.is_ip_address(input) {
            return true;
        }

        // Domain-like pattern
        if input.contains('.') {
            let parts: Vec<&str> = input.split('.').collect();
            if parts.len() >= 2 {
                // Check for valid TLD
                let tld = parts.last().unwrap();
                // Split off port/path if present
                let tld = tld.split(':').next().unwrap();
                let tld = tld.split('/').next().unwrap();

                // Basic TLD validation (2-6 chars)
                if tld.len() >= 2 && tld.len() <= 6 && tld.chars().all(|c| c.is_alphabetic()) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if input looks like an IP address
    fn is_ip_address(&self, input: &str) -> bool {
        let (host, _) = Self::split_host_and_rest(input);
        self.parse_ip_host(host).is_some()
    }

    fn is_ipv6_host(&self, host: &str) -> bool {
        matches!(self.parse_ip_host(host), Some(IpAddr::V6(_)))
    }

    fn parse_ip_host(&self, host: &str) -> Option<IpAddr> {
        let host = host.trim();
        if host.is_empty() {
            return None;
        }

        let host = if host.starts_with('[') {
            host.strip_prefix('[')
                .and_then(|s| s.split(']').next())
                .unwrap_or(host)
        } else if host.matches(':').count() == 1 {
            host.split(':').next().unwrap_or(host)
        } else {
            host
        };

        host.parse().ok()
    }

    fn split_host_and_rest(input: &str) -> (&str, &str) {
        let mut cut = input.len();
        for ch in ['/', '?', '#'] {
            if let Some(idx) = input.find(ch) {
                if idx < cut {
                    cut = idx;
                }
            }
        }

        input.split_at(cut)
    }

    /// Build search URL from query
    fn build_search_url(&self, query: &str) -> String {
        let encoded = urlencoding::encode(query);
        self.search_template.replace("%s", &encoded)
    }
}

impl Default for InputResolver {
    fn default() -> Self {
        Self::new()
    }
}

// Add urlencoding as a simple inline implementation
mod urlencoding {
    pub fn encode(input: &str) -> String {
        let mut result = String::with_capacity(input.len() * 3);
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push_str(&format!("%{:02X}", byte));
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_url() {
        let resolver = InputResolver::new();

        // Full URL
        match resolver.resolve("https://example.com") {
            InputResolution::Navigate(url) => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }

        // Domain only
        match resolver.resolve("example.com") {
            InputResolution::Navigate(url) => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected Navigate"),
        }

        // localhost
        match resolver.resolve("localhost:8080") {
            InputResolution::Navigate(url) => assert_eq!(url, "https://localhost:8080"),
            _ => panic!("Expected Navigate"),
        }
    }

    #[test]
    fn test_resolve_search() {
        let resolver = InputResolver::new();

        match resolver.resolve("rust programming") {
            InputResolution::Search(url) => {
                assert!(url.contains("duckduckgo.com"));
                assert!(url.contains("rust%20programming"));
            }
            _ => panic!("Expected Search"),
        }
    }

    #[test]
    fn test_resolve_command() {
        let resolver = InputResolver::new();

        match resolver.resolve("@tabs github") {
            InputResolution::Command(cmd) => {
                assert_eq!(cmd.query, Some("github".to_string()));
            }
            _ => panic!("Expected Command"),
        }
    }

    #[test]
    fn test_resolve_ipv6() {
        let resolver = InputResolver::new();

        match resolver.resolve("::1") {
            InputResolution::Navigate(url) => assert_eq!(url, "https://[::1]"),
            _ => panic!("Expected Navigate"),
        }

        match resolver.resolve("[::1]:8080") {
            InputResolution::Navigate(url) => assert_eq!(url, "https://[::1]:8080"),
            _ => panic!("Expected Navigate"),
        }

        match resolver.resolve("2001:db8::1/path") {
            InputResolution::Navigate(url) => assert_eq!(url, "https://[2001:db8::1]/path"),
            _ => panic!("Expected Navigate"),
        }
    }
}

//! Command system for address bar
//!
//! Per PRD Section 5.3:
//! - `@tabs` — fuzzy search open tabs
//! - `@history` — fuzzy search history
//! - `@sessions` — switch session

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandType {
    /// Search open tabs
    Tabs,
    /// Search history
    History,
    /// Switch sessions
    Sessions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub command_type: CommandType,
    /// Optional search query after the command
    pub query: Option<String>,
}

impl Command {
    /// Parse a command string (must start with @)
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();
        if !input.starts_with('@') {
            return None;
        }

        let without_prefix = &input[1..];
        let mut parts = without_prefix.splitn(2, ' ');
        let command = parts.next()?.to_lowercase();
        let query = parts
            .next()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let command_type = match command.as_str() {
            "tabs" | "tab" | "t" => CommandType::Tabs,
            "history" | "hist" | "h" => CommandType::History,
            "sessions" | "session" | "s" => CommandType::Sessions,
            _ => return None,
        };

        Some(Self {
            command_type,
            query,
        })
    }

    /// Get the command prefix for display
    pub fn prefix(&self) -> &'static str {
        match self.command_type {
            CommandType::Tabs => "@tabs",
            CommandType::History => "@history",
            CommandType::Sessions => "@sessions",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tabs() {
        let cmd = Command::parse("@tabs").unwrap();
        assert_eq!(cmd.command_type, CommandType::Tabs);
        assert!(cmd.query.is_none());

        let cmd = Command::parse("@tabs github").unwrap();
        assert_eq!(cmd.command_type, CommandType::Tabs);
        assert_eq!(cmd.query, Some("github".to_string()));
    }

    #[test]
    fn test_parse_shortcuts() {
        let cmd = Command::parse("@t").unwrap();
        assert_eq!(cmd.command_type, CommandType::Tabs);

        let cmd = Command::parse("@h").unwrap();
        assert_eq!(cmd.command_type, CommandType::History);

        let cmd = Command::parse("@s").unwrap();
        assert_eq!(cmd.command_type, CommandType::Sessions);
    }

    #[test]
    fn test_unknown_command() {
        assert!(Command::parse("@unknown").is_none());
        assert!(Command::parse("not a command").is_none());
    }
}

//! History management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::Result;
use axiom_storage::Database;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: DateTime<Utc>,
    pub visit_count: i32,
}

pub struct HistoryManager {
    db: Database,
}

impl HistoryManager {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Record a visit to a URL
    pub fn record_visit(&self, url: &str, title: &str) -> Result<()> {
        Ok(self.db.with_connection(|conn| {
            // Check if URL exists
            let existing: Option<i64> = conn.query_row(
                "SELECT id FROM history WHERE url = ?1",
                [url],
                |row| row.get(0),
            ).ok();

            if let Some(id) = existing {
                // Update existing entry
                conn.execute(
                    "UPDATE history
                     SET title = CASE WHEN ?1 != '' THEN ?1 ELSE title END,
                         visited_at = ?2,
                         visit_count = visit_count + 1
                     WHERE id = ?3",
                    rusqlite::params![title, Utc::now().to_rfc3339(), id],
                )?;
            } else {
                // Insert new entry
                conn.execute(
                    "INSERT INTO history (url, title, visited_at, visit_count) VALUES (?1, ?2, ?3, 1)",
                    rusqlite::params![url, title, Utc::now().to_rfc3339()],
                )?;
            }

            Ok(())
        })?)
    }

    /// Update the stored title for a URL without incrementing visit count.
    pub fn update_title(&self, url: &str, title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return Ok(());
        }

        Ok(self.db.with_connection(|conn| {
            conn.execute(
                "UPDATE history SET title = ?1 WHERE url = ?2",
                rusqlite::params![title, url],
            )?;
            Ok(())
        })?)
    }

    /// Search history by query
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<HistoryEntry>> {
        Ok(self.db.with_connection(|conn| {
            let pattern = format!("%{}%", query.to_lowercase());

            let mut stmt = conn.prepare(
                "SELECT id, url, title, visited_at, visit_count FROM history    
                 WHERE LOWER(url) LIKE ?1 OR LOWER(title) LIKE ?1
                 ORDER BY visited_at DESC, visit_count DESC
                 LIMIT ?2",
            )?;

            let entries: Vec<HistoryEntry> = stmt
                .query_map(rusqlite::params![pattern, limit as i64], |row| {
                    let visited_str: String = row.get(3)?;
                    let visited_at = DateTime::parse_from_rfc3339(&visited_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(HistoryEntry {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        title: row.get(2)?,
                        visited_at,
                        visit_count: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(entries)
        })?)
    }

    /// Get recent history entries
    pub fn recent(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        Ok(self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, url, title, visited_at, visit_count FROM history
                 ORDER BY visited_at DESC
                 LIMIT ?1",
            )?;

            let entries: Vec<HistoryEntry> = stmt
                .query_map([limit as i64], |row| {
                    let visited_str: String = row.get(3)?;
                    let visited_at = DateTime::parse_from_rfc3339(&visited_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now());

                    Ok(HistoryEntry {
                        id: row.get(0)?,
                        url: row.get(1)?,
                        title: row.get(2)?,
                        visited_at,
                        visit_count: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(entries)
        })?)
    }

    /// Delete a history entry
    pub fn delete(&self, id: i64) -> Result<()> {
        Ok(self.db.with_connection(|conn| {
            conn.execute("DELETE FROM history WHERE id = ?1", [id])?;
            Ok(())
        })?)
    }

    /// Clear all history
    pub fn clear_all(&self) -> Result<()> {
        Ok(self.db.with_connection(|conn| {
            conn.execute("DELETE FROM history", [])?;
            Ok(())
        })?)
    }

    /// Clear history within an optional time range (inclusive).
    pub fn clear_range(
        &self,
        start: Option<DateTime<Utc>>,
        end: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let start = start.map(|t| t.to_rfc3339());
        let end = end.map(|t| t.to_rfc3339());

        Ok(self.db.with_connection(|conn| {
            match (start, end) {
                (Some(start), Some(end)) => {
                    conn.execute(
                        "DELETE FROM history WHERE visited_at >= ?1 AND visited_at <= ?2",
                        rusqlite::params![start, end],
                    )?;
                }
                (Some(start), None) => {
                    conn.execute("DELETE FROM history WHERE visited_at >= ?1", [start])?;
                }
                (None, Some(end)) => {
                    conn.execute("DELETE FROM history WHERE visited_at <= ?1", [end])?;
                }
                (None, None) => {
                    conn.execute("DELETE FROM history", [])?;
                }
            }
            Ok(())
        })?)
    }
}

impl Clone for HistoryManager {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_manager() {
        let db = Database::open_in_memory().unwrap();
        let manager = HistoryManager::new(db);

        // Record visits
        manager
            .record_visit("https://example.com", "Example")
            .unwrap();
        manager
            .record_visit("https://rust-lang.org", "Rust")
            .unwrap();
        manager
            .record_visit("https://example.com", "Example")
            .unwrap(); // Second visit

        // Search
        let results = manager.search("example", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].visit_count, 2);

        // Recent
        let recent = manager.recent(10).unwrap();
        assert_eq!(recent.len(), 2);
    }
}

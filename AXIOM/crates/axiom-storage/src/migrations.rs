//! Database migrations
//!
//! Schema per PRD Section 9: tabs, sessions, history, settings, downloads

use crate::Result;
use rusqlite::Connection;

const SCHEMA_VERSION: i32 = 1;

pub fn run_migrations(conn: &Connection) -> Result<()> {
    let current_version = get_schema_version(conn)?;

    if current_version < 1 {
        migrate_v1(conn)?;
    }

    set_schema_version(conn, SCHEMA_VERSION)?;
    Ok(())
}

fn get_schema_version(conn: &Connection) -> Result<i32> {
    let result: std::result::Result<i32, _> =
        conn.query_row("SELECT version FROM schema_version LIMIT 1", [], |row| {
            row.get(0)
        });

    match result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(rusqlite::Error::SqliteFailure(_, _)) => {
            // Table doesn't exist yet
            conn.execute(
                "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL)",
                [],
            )?;
            conn.execute("INSERT INTO schema_version (version) VALUES (0)", [])?;
            Ok(0)
        }
        Err(e) => Err(e.into()),
    }
}

fn set_schema_version(conn: &Connection, version: i32) -> Result<()> {
    conn.execute("DELETE FROM schema_version", [])?;
    conn.execute(
        "INSERT INTO schema_version (version) VALUES (?1)",
        [version],
    )?;
    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<()> {
    tracing::info!("Running migration v1: Initial schema");

    // Sessions table - first-class objects per PRD
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0,
            tab_order TEXT NOT NULL DEFAULT '[]'
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_active ON sessions(is_active);
    "#,
    )?;

    // Tabs table with state machine states per PRD Section 5.1
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS tabs (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            url TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            favicon_url TEXT,
            state TEXT NOT NULL DEFAULT 'active',
            scroll_position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_accessed_at TEXT NOT NULL,
            snapshot_path TEXT,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_tabs_session ON tabs(session_id);
        CREATE INDEX IF NOT EXISTS idx_tabs_state ON tabs(state);
    "#,
    )?;

    // History table
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            visited_at TEXT NOT NULL,
            visit_count INTEGER NOT NULL DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);
        CREATE INDEX IF NOT EXISTS idx_history_visited ON history(visited_at);
    "#,
    )?;

    // Settings table
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
    "#,
    )?;

    // Downloads table per PRD Section 5.5
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS downloads (
            id TEXT PRIMARY KEY,
            url TEXT NOT NULL,
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            mime_type TEXT,
            total_bytes INTEGER,
            downloaded_bytes INTEGER NOT NULL DEFAULT 0,
            state TEXT NOT NULL DEFAULT 'pending',
            hash TEXT,
            created_at TEXT NOT NULL,
            completed_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_downloads_state ON downloads(state);
    "#,
    )?;

    Ok(())
}

//! Path configuration for tt
//!
//! Provides centralized path management for the tt directory, database, and artifacts.

use std::path::PathBuf;

/// Name of the tt hidden directory
pub const TT_DIR: &str = ".tt";

/// Name of the database file (inside .tt directory)
pub const DB_FILE: &str = "tt.db";

/// Name of the artifacts subdirectory
pub const ARTIFACTS_DIR: &str = "artifacts";

/// Get the path to the tt directory (./.tt/)
pub fn get_tt_dir() -> PathBuf {
    PathBuf::from(TT_DIR)
}

/// Get the path to the database file (./.tt/tt.db)
pub fn get_db_path() -> PathBuf {
    get_tt_dir().join(DB_FILE)
}

/// Get the path to the artifacts directory relative to the found database
pub fn get_artifacts_dir() -> Option<PathBuf> {
    find_db_path().map(|p| p.parent().unwrap().join(ARTIFACTS_DIR))
}

/// Find the database path by searching upward from current directory
///
/// This allows tt to work correctly when invoked from different directories
/// (e.g., when run as an MCP server from an AI tool that changes CWD).
pub fn find_db_path() -> Option<PathBuf> {
    let mut path = std::env::current_dir().ok()?;

    loop {
        // Look for .tt/tt.db (the standard layout)
        let db_path = path.join(TT_DIR).join(DB_FILE);
        if db_path.exists() {
            return Some(db_path);
        }

        // Don't search above the filesystem root
        if !path.pop() {
            return None;
        }
    }
}

/// Check if the tt project is initialized (database exists somewhere in cwd tree)
pub fn is_initialized() -> bool {
    find_db_path().is_some()
}

/// Ensure the database exists and return a connection
///
/// Searches upward from CWD to find the database. Exits with error if not found.
pub fn ensure_db() -> crate::error::Result<rusqlite::Connection> {
    match find_db_path() {
        Some(path) => crate::db::open_db(path),
        None => {
            eprintln!("Error: No tt project found. Run `tt init` first.");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tt_dir() {
        let dir = get_tt_dir();
        assert_eq!(dir, PathBuf::from(".tt"));
    }

    #[test]
    fn test_get_db_path() {
        let db = get_db_path();
        assert_eq!(db, PathBuf::from(".tt/tt.db"));
    }
}

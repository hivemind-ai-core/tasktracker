//! Dependency commands

use crate::core::{add_dependency, remove_dependency};
use crate::error::Result;
use rusqlite::Connection;

/// Add a dependency
pub fn run_depend(conn: &Connection, id: i64, on_id: i64) -> Result<()> {
    add_dependency(conn, id, on_id)?;

    println!("Added dependency: #{id} depends on #{on_id}");

    Ok(())
}

/// Remove a dependency
pub fn run_undepend(conn: &Connection, id: i64, on_id: i64) -> Result<()> {
    remove_dependency(conn, id, on_id)?;

    println!("Removed dependency: #{id} no longer depends on #{on_id}");

    Ok(())
}

//! Dependency commands

use crate::core::{add_dependencies, add_dependency, remove_dependencies, remove_dependency};
use crate::error::Result;
use rusqlite::Connection;

/// Manage dependencies (add or remove)
pub fn run_depend(conn: &Connection, id: i64, on_ids: Vec<i64>, remove: bool) -> Result<()> {
    if remove {
        // Remove dependencies
        if on_ids.len() == 1 {
            // Single dependency
            remove_dependency(conn, id, on_ids[0])?;
            println!(
                "Removed dependency: #{id} no longer depends on #{}",
                on_ids[0]
            );
        } else {
            // Bulk dependencies
            remove_dependencies(conn, id, on_ids.clone())?;
            let ids_str: Vec<String> = on_ids.iter().map(|i| format!("#{i}")).collect();
            println!(
                "Removed dependencies: #{id} no longer depends on {}",
                ids_str.join(", ")
            );
        }
    } else {
        // Add dependencies
        if on_ids.len() == 1 {
            // Single dependency
            add_dependency(conn, id, on_ids[0])?;
            println!("Added dependency: #{id} depends on #{}", on_ids[0]);
        } else {
            // Bulk dependencies
            add_dependencies(conn, id, on_ids.clone())?;
            let ids_str: Vec<String> = on_ids.iter().map(|i| format!("#{i}")).collect();
            println!(
                "Added dependencies: #{id} depends on {}",
                ids_str.join(", ")
            );
        }
    }

    Ok(())
}

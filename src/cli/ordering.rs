//! Ordering commands

use crate::core::{reindex_tasks, reorder_task};
use crate::error::Result;
use rusqlite::Connection;

/// Reorder a task
pub fn run_reorder(
    conn: &Connection,
    id: i64,
    after: Option<i64>,
    before: Option<i64>,
) -> Result<()> {
    if after.is_none() && before.is_none() {
        eprintln!("Error: Must specify --after or --before");
        std::process::exit(1);
    }

    let new_order = reorder_task(conn, id, after, before)?;

    println!("Reordered task #{id}. New order: {new_order}");

    Ok(())
}

/// Reindex all tasks
pub fn run_reindex(conn: &Connection) -> Result<()> {
    let new_orders = reindex_tasks(conn)?;

    println!("Reindexed {} tasks.", new_orders.len());
    for (id, order) in new_orders {
        println!("  #{id} -> {order}");
    }

    Ok(())
}

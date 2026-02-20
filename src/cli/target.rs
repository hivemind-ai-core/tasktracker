//! Target commands

use crate::core::{get_next_task, get_target, set_target, TaskStatus};
use crate::error::{Error, Result};
use rusqlite::Connection;

/// Set the target task
pub fn run_set(conn: &Connection, id: i64) -> Result<()> {
    set_target(conn, id)?;

    println!("Set target to #{id}.");

    Ok(())
}

/// Show next task
pub fn run_next(conn: &Connection) -> Result<()> {
    match get_next_task(conn)? {
        Some(task) => {
            println!("Next: [#{}] {}", task.id, task.title);

            // Show dependencies
            let deps = crate::db::dependencies::get_dependencies(conn, task.id)?;
            if !deps.is_empty() {
                let mut dep_info = Vec::new();
                let mut all_met = true;

                for dep in deps {
                    if let Some(t) = crate::db::tasks::get_task(conn, dep.depends_on)? {
                        let status = if t.status == TaskStatus::Completed {
                            "✓"
                        } else {
                            "○"
                        };
                        dep_info.push(format!("#{} {}", t.id, status));
                        if t.status != TaskStatus::Completed {
                            all_met = false;
                        }
                    }
                }

                let status = if all_met { "all met" } else { "not all met" };
                println!("  Dependencies: {} ({})", dep_info.join(", "), status);
            }

            // Show DoD if exists
            if let Some(ref dod) = task.dod {
                println!("  DoD: {dod}");
            }
        }
        None => {
            // Check if we have a target
            match get_target(conn)? {
                Some(target) => {
                    println!(
                        "Target Reached: all tasks for #{} ({}) are completed.",
                        target.id, target.title
                    );
                }
                None => {
                    return Err(Error::NoTarget);
                }
            }
        }
    }

    Ok(())
}

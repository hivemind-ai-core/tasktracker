//! Focus commands

use crate::core::{clear_target, get_next_task, get_target, set_target, TaskStatus};
use crate::error::Result;
use rusqlite::Connection;

/// Show the current focus
pub fn run_show(conn: &Connection) -> Result<()> {
    match get_target(conn)? {
        Some(target) => {
            println!("Focus: [#{}] {}", target.id, target.title);
            println!("  Status: {}", target.status);
        }
        None => {
            println!("No focus set.");
        }
    }
    Ok(())
}

/// Set the focus task
pub fn run_set(conn: &Connection, id: i64) -> Result<()> {
    set_target(conn, id)?;

    println!("Set focus to #{id}.");

    Ok(())
}

/// Clear the focus
pub fn run_clear(conn: &Connection) -> Result<()> {
    clear_target(conn)?;
    println!("Focus cleared.");
    Ok(())
}

/// Set focus to next incomplete task (lowest manual_order pending)
pub fn run_target_next(conn: &Connection) -> Result<()> {
    let all_tasks = crate::db::tasks::get_all_tasks(conn)?;

    // Find the pending task with lowest manual_order
    let next_task = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .min_by(|a, b| {
            a.manual_order
                .partial_cmp(&b.manual_order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

    match next_task {
        Some(task) => {
            set_target(conn, task.id)?;
            println!("Set focus to #{} ({})", task.id, task.title);
            Ok(())
        }
        None => {
            println!("No pending tasks available.");
            Ok(())
        }
    }
}

/// Set focus to most recent task (highest id)
pub fn run_target_last(conn: &Connection) -> Result<()> {
    let all_tasks = crate::db::tasks::get_all_tasks(conn)?;

    match all_tasks.iter().max_by_key(|t| t.id) {
        Some(task) => {
            set_target(conn, task.id)?;
            println!("Set focus to #{} ({})", task.id, task.title);
            Ok(())
        }
        None => {
            println!("No tasks available.");
            Ok(())
        }
    }
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
                    if let Some(t) = crate::db::tasks::get_task(conn, dep.depends_on, false)? {
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
            // Check if we have a focus
            match get_target(conn)? {
                Some(target) => {
                    println!(
                        "Focus Reached: all tasks for #{} ({}) are completed.",
                        target.id, target.title
                    );
                }
                None => {
                    println!("No pending tasks available.");
                }
            }
        }
    }

    Ok(())
}

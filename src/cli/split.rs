//! Split task command

use crate::core::split_task;
use crate::error::Result;
use rusqlite::Connection;

/// Split a task into multiple subtasks
///
/// Parts are specified as groups of 3 strings: title, description, dod
/// Example: tt split 1 "Subtask 1" "Desc 1" "DoD 1" "Subtask 2" "Desc 2" "DoD 2"
pub fn run(conn: &Connection, task_id: i64, parts: Vec<String>) -> Result<()> {
    // Validate that we have groups of 3
    if parts.len() % 3 != 0 {
        eprintln!("Error: Arguments must be in groups of 3 (title, description, dod)");
        eprintln!("Example: tt split 1 \"Subtask 1\" \"Desc 1\" \"DoD 1\" \"Subtask 2\" \"Desc 2\" \"DoD 2\"");
        std::process::exit(1);
    }

    let num_subtasks = parts.len() / 3;
    if num_subtasks == 0 {
        eprintln!("Error: At least one subtask definition is required");
        std::process::exit(1);
    }

    // Parse parts into subtask tuples
    let mut subtasks = Vec::with_capacity(num_subtasks);
    for i in 0..num_subtasks {
        let title = parts[i * 3].clone();
        let desc = parts[i * 3 + 1].clone();
        let dod = parts[i * 3 + 2].clone();
        subtasks.push((title, desc, dod));
    }

    // Perform the split
    let new_tasks = split_task(conn, task_id, subtasks)?;

    // Output results
    println!("Split task #{} into {} subtasks:", task_id, new_tasks.len());
    for task in &new_tasks {
        println!("  Created: [#{}] {}", task.id, task.title);
    }

    Ok(())
}

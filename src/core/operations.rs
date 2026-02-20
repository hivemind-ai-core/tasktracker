//! High-level operations combining core logic with database
//!
//! This module provides the API that CLI and MCP layers use.
//! All operations enforce business rules and invariants.

use crate::core::models::{Artifact, Dependency, Task, TaskDetail, TaskStatus};
use crate::core::{
    calculate_manual_order, can_block_task, can_complete_task, can_start_task, can_stop_task,
    can_unblock_task, find_next_task as find_next, get_blocked_tasks, is_target_reached,
    reindex_orders, would_create_cycle,
};
use crate::db;
use crate::error::{Error, Result};
use rusqlite::Connection;

/// Create a new task with automatic order calculation
pub fn create_task(
    conn: &Connection,
    title: &str,
    description: Option<&str>,
    dod: Option<&str>,
    after_id: Option<i64>,
    before_id: Option<i64>,
) -> Result<Task> {
    // Calculate manual order
    let all_tasks = db::tasks::get_all_tasks(conn)?;
    let manual_order = calculate_manual_order(&all_tasks, after_id, before_id)?;

    // Create the task
    db::tasks::create_task(conn, title, description, dod, manual_order)
}

/// Edit task fields
pub fn edit_task(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    dod: Option<&str>,
) -> Result<Task> {
    // Update provided fields
    if let Some(t) = title {
        db::tasks::update_task_title(conn, id, t)?;
    }
    if let Some(d) = description {
        db::tasks::update_task_description(conn, id, Some(d))?;
    }
    if let Some(d) = dod {
        db::tasks::update_task_dod(conn, id, Some(d))?;
    }

    // Return updated task
    db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))
}

/// Get task details with relationships
pub fn get_task_detail(conn: &Connection, id: i64) -> Result<TaskDetail> {
    let task = db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))?;

    // Get dependencies (prerequisites)
    let dep_records = db::dependencies::get_dependencies(conn, id)?;
    let mut dependencies = Vec::new();
    for dep in dep_records {
        if let Some(t) = db::tasks::get_task(conn, dep.depends_on)? {
            dependencies.push(t);
        }
    }

    // Get dependents (tasks that depend on this one)
    let dependent_records = db::dependencies::get_dependents(conn, id)?;
    let mut dependents = Vec::new();
    for dep in dependent_records {
        if let Some(t) = db::tasks::get_task(conn, dep.task_id)? {
            dependents.push(t);
        }
    }

    // Get artifacts
    let artifacts = db::artifacts::get_artifacts_for_task(conn, id)?;

    Ok(TaskDetail {
        task,
        dependencies,
        dependents,
        artifacts,
    })
}

/// Start a task with all guards
pub fn start_task(conn: &Connection, id: i64) -> Result<Task> {
    let task = db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))?;

    // Get active task (if any)
    let active_task = db::tasks::get_active_task(conn)?;

    // Get incomplete dependencies
    let incomplete_deps = db::dependencies::get_incomplete_dependencies(conn, id)?;

    // Validate transition
    can_start_task(&task, active_task.as_ref(), &incomplete_deps)?;

    // If already in progress, return as no-op
    if task.status == TaskStatus::InProgress {
        return Ok(task);
    }

    // Perform the transition
    db::tasks::start_task(conn, id)?;

    // Return updated task
    db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))
}

/// Stop the active task
pub fn stop_task(conn: &Connection) -> Result<Task> {
    // Get active task
    let task = db::tasks::get_active_task(conn)?.ok_or(Error::NoActiveTask)?;

    // Validate
    can_stop_task(&task)?;

    // Perform transition
    db::tasks::stop_task(conn, task.id)?;

    // Return updated task
    db::tasks::get_task(conn, task.id)?.ok_or(Error::TaskNotFound(task.id))
}

/// Complete the active task
pub fn complete_task(conn: &Connection) -> Result<Task> {
    // Get active task
    let task = db::tasks::get_active_task(conn)?.ok_or(Error::NoActiveTask)?;

    eprintln!(
        "DEBUG complete_task: task={}, status={}, dod={:?}",
        task.id, task.status, task.dod
    );

    // Validate
    can_complete_task(&task)?;

    // Perform transition
    db::tasks::complete_task(conn, task.id)?;

    // Return updated task
    db::tasks::get_task(conn, task.id)?.ok_or(Error::TaskNotFound(task.id))
}

/// Block a task
pub fn block_task(conn: &Connection, id: i64) -> Result<Task> {
    let task = db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))?;

    // Validate
    can_block_task(&task)?;

    // Perform transition
    db::tasks::block_task(conn, id)?;

    // Return updated task
    db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))
}

/// Unblock a task
pub fn unblock_task(conn: &Connection, id: i64) -> Result<Task> {
    let task = db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))?;

    // Validate
    can_unblock_task(&task)?;

    // Perform transition
    db::tasks::unblock_task(conn, id)?;

    // Return updated task
    db::tasks::get_task(conn, id)?.ok_or(Error::TaskNotFound(id))
}

/// Add a dependency with cycle detection
pub fn add_dependency(conn: &Connection, task_id: i64, depends_on: i64) -> Result<()> {
    // Check if tasks exist
    if db::tasks::get_task(conn, task_id)?.is_none() {
        return Err(Error::TaskNotFound(task_id));
    }
    if db::tasks::get_task(conn, depends_on)?.is_none() {
        return Err(Error::TaskNotFound(depends_on));
    }

    // Get all existing dependencies
    let all_deps = db::dependencies::get_all_dependencies(conn)?;

    // Check for cycle
    if would_create_cycle(task_id, depends_on, &all_deps) {
        // Find the cycle path for the error message
        use crate::core::find_cycle_path;
        let cycle_path = find_cycle_path(task_id, depends_on, &all_deps)
            .unwrap_or_else(|| vec![task_id, depends_on, task_id]);
        return Err(Error::CycleDetected(task_id, depends_on, cycle_path));
    }

    // Add the dependency
    db::dependencies::add_dependency(conn, task_id, depends_on)
}

/// Remove a dependency
pub fn remove_dependency(conn: &Connection, task_id: i64, depends_on: i64) -> Result<()> {
    db::dependencies::remove_dependency(conn, task_id, depends_on)
}

/// Set the target task
pub fn set_target(conn: &Connection, target_id: i64) -> Result<()> {
    // Verify task exists
    if db::tasks::get_task(conn, target_id)?.is_none() {
        return Err(Error::TaskNotFound(target_id));
    }

    db::config::set_target(conn, target_id)
}

/// Get the current target
pub fn get_target(conn: &Connection) -> Result<Option<Task>> {
    match db::config::get_target(conn)? {
        Some(id) => db::tasks::get_task(conn, id),
        None => Ok(None),
    }
}

/// Get the next task to work on
pub fn get_next_task(conn: &Connection) -> Result<Option<Task>> {
    // Check if target is set
    let target_id = db::config::get_target(conn)?.ok_or(Error::NoTarget)?;

    // Get all tasks and dependencies
    let all_tasks = db::tasks::get_all_tasks(conn)?;
    let all_deps = db::dependencies::get_all_dependencies(conn)?;

    // Check if target is reached
    if is_target_reached(target_id, &all_tasks, &all_deps) {
        return Err(Error::TargetReached(target_id));
    }

    // Check if all remaining are blocked
    let blocked = get_blocked_tasks(target_id, &all_tasks, &all_deps);
    let remaining: Vec<_> = all_tasks
        .iter()
        .filter(|t| {
            t.status != TaskStatus::Completed
                && is_in_target_subgraph(t.id, target_id, &all_deps, &all_tasks)
        })
        .collect();

    if !remaining.is_empty() && remaining.iter().all(|t| t.status == TaskStatus::Blocked) {
        let blocked_ids: Vec<i64> = blocked.iter().map(|t| t.id).collect();
        return Err(Error::AllBlocked(blocked_ids));
    }

    // Find next ready task
    Ok(find_next(target_id, &all_tasks, &all_deps))
}

/// Check if a task is in the target subgraph
fn is_in_target_subgraph(
    task_id: i64,
    target_id: i64,
    deps: &[Dependency],
    _tasks: &[Task],
) -> bool {
    if task_id == target_id {
        return true;
    }

    // Build a map of task -> what depends on it
    let mut dependents: std::collections::HashMap<i64, Vec<i64>> = std::collections::HashMap::new();
    for dep in deps {
        dependents
            .entry(dep.depends_on)
            .or_default()
            .push(dep.task_id);
    }

    // BFS from target to see if we can reach task_id
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![target_id];

    while let Some(current) = stack.pop() {
        if current == task_id {
            return true;
        }

        if visited.insert(current) {
            if let Some(deps) = dependents.get(&current) {
                for &dep in deps {
                    stack.push(dep);
                }
            }
        }
    }

    false
}

/// List tasks in the target subgraph (or all tasks)
pub fn list_tasks(conn: &Connection, all: bool) -> Result<Vec<Task>> {
    if all {
        db::tasks::get_all_tasks(conn)
    } else {
        // Get target
        let target_id = db::config::get_target(conn)?.ok_or(Error::NoTarget)?;

        // Get subgraph
        let all_tasks = db::tasks::get_all_tasks(conn)?;
        let all_deps = db::dependencies::get_all_dependencies(conn)?;
        let subgraph = crate::core::compute_target_subgraph(target_id, &all_tasks, &all_deps);

        // Sort topologically (dependency order), with manual_order as tiebreaker
        let sorted = crate::core::topological_sort(subgraph, &all_deps).unwrap_or_else(|_| {
            // Fallback to manual_order if there's a cycle
            let mut sorted = all_tasks;
            sorted.sort_by(|a, b| {
                a.manual_order
                    .partial_cmp(&b.manual_order)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            sorted
        });

        Ok(sorted)
    }
}

/// Reorder a task
pub fn reorder_task(
    conn: &Connection,
    id: i64,
    after_id: Option<i64>,
    before_id: Option<i64>,
) -> Result<f64> {
    // Get all tasks except this one for order calculation
    let all_tasks = db::tasks::get_all_tasks(conn)?;
    let other_tasks: Vec<_> = all_tasks.into_iter().filter(|t| t.id != id).collect();

    // Calculate new order
    let new_order = calculate_manual_order(&other_tasks, after_id, before_id)?;

    // Update the task
    db::tasks::update_task_order(conn, id, new_order)?;

    Ok(new_order)
}

/// Reindex all task orders
pub fn reindex_tasks(conn: &Connection) -> Result<Vec<(i64, f64)>> {
    let all_tasks = db::tasks::get_all_tasks(conn)?;
    let new_orders = reindex_orders(&all_tasks);

    // Update each task
    for (id, order) in &new_orders {
        db::tasks::update_task_order(conn, *id, *order)?;
    }

    Ok(new_orders)
}

/// Log an artifact for the active task
pub fn log_artifact(conn: &Connection, name: &str, file_path: &str) -> Result<Artifact> {
    // Get active task
    let task = db::tasks::get_active_task(conn)?.ok_or(Error::NoActiveTask)?;

    // Create artifact
    db::artifacts::create_artifact(conn, task.id, name, file_path)
}

/// Get artifacts for a task (defaults to active task)
pub fn get_task_artifacts(conn: &Connection, task_id: Option<i64>) -> Result<Vec<Artifact>> {
    let id = match task_id {
        Some(id) => id,
        None => {
            db::tasks::get_active_task(conn)?
                .ok_or(Error::NoActiveTask)?
                .id
        }
    };

    db::artifacts::get_artifacts_for_task(conn, id)
}

/// Get the current active task
pub fn get_current_task(conn: &Connection) -> Result<Task> {
    db::tasks::get_active_task(conn)?.ok_or(Error::NoActiveTask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::open_memory_db;
    use crate::db::schema::CREATE_SCHEMA_SQL;

    fn setup() -> Connection {
        let conn = open_memory_db().unwrap();
        conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();
        conn
    }

    #[test]
    fn test_create_task() {
        let conn = setup();
        let task = create_task(&conn, "Test Task", None, None, None, None).unwrap();

        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_create_task_with_order() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", None, None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", None, None, Some(task1.id), None).unwrap();

        assert!(task2.manual_order > task1.manual_order);
    }

    #[test]
    fn test_edit_task() {
        let conn = setup();
        let task = create_task(&conn, "Old Title", None, None, None, None).unwrap();

        let updated =
            edit_task(&conn, task.id, Some("New Title"), Some("Desc"), Some("DoD")).unwrap();

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.description, Some("Desc".to_string()));
        assert_eq!(updated.dod, Some("DoD".to_string()));
    }

    #[test]
    fn test_get_task_detail() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", None, None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", None, None, None, None).unwrap();

        // Add dependency
        add_dependency(&conn, task2.id, task1.id).unwrap();

        let detail = get_task_detail(&conn, task2.id).unwrap();

        assert_eq!(detail.dependencies.len(), 1);
        assert_eq!(detail.dependents.len(), 0);

        let detail1 = get_task_detail(&conn, task1.id).unwrap();
        assert_eq!(detail1.dependents.len(), 1);
    }

    #[test]
    fn test_start_task_workflow() {
        let conn = setup();
        let task = create_task(&conn, "Task", None, Some("DoD"), None, None).unwrap();

        let started = start_task(&conn, task.id).unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);

        let completed = complete_task(&conn).unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
    }

    #[test]
    fn test_cannot_complete_without_dod() {
        let conn = setup();
        let task = create_task(&conn, "Task", None, None, None, None).unwrap();
        start_task(&conn, task.id).unwrap();

        let result = complete_task(&conn);
        assert!(matches!(result.unwrap_err(), Error::NoDod(_)));
    }

    #[test]
    fn test_cannot_start_with_unmet_deps() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", None, None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", None, None, None, None).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();

        let result = start_task(&conn, task2.id);
        assert!(matches!(
            result.unwrap_err(),
            Error::UnmetDependencies(_, _)
        ));
    }

    #[test]
    fn test_cycle_detection() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", None, None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", None, None, None, None).unwrap();
        let task3 = create_task(&conn, "Task 3", None, None, None, None).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();
        add_dependency(&conn, task3.id, task2.id).unwrap();

        // Adding 1 -> 3 would create a cycle
        let result = add_dependency(&conn, task1.id, task3.id);
        assert!(matches!(result.unwrap_err(), Error::CycleDetected(_, _, _)));
    }

    #[test]
    fn test_target_operations() {
        let conn = setup();
        let task = create_task(&conn, "Target", None, None, None, None).unwrap();

        set_target(&conn, task.id).unwrap();

        let target = get_target(&conn).unwrap();
        assert!(target.is_some());
        assert_eq!(target.unwrap().id, task.id);

        // Without completing task, next should return the task
        let next = get_next_task(&conn);
        assert!(next.is_ok());
        assert_eq!(next.unwrap().unwrap().id, task.id);
    }

    #[test]
    fn test_target_reached() {
        let conn = setup();
        let task = create_task(&conn, "Target", None, Some("DoD"), None, None).unwrap();

        set_target(&conn, task.id).unwrap();
        start_task(&conn, task.id).unwrap();
        complete_task(&conn).unwrap();

        let result = get_next_task(&conn);
        assert!(matches!(result.unwrap_err(), Error::TargetReached(_)));
    }

    #[test]
    fn test_reorder_task() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", None, None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", None, None, None, None).unwrap();

        // Reorder task2 to be before task1
        let new_order = reorder_task(&conn, task2.id, None, Some(task1.id)).unwrap();

        let updated = db::tasks::get_task(&conn, task2.id).unwrap().unwrap();
        assert_eq!(updated.manual_order, new_order);
        assert!(updated.manual_order < task1.manual_order);
    }

    #[test]
    fn test_reindex_tasks() {
        let conn = setup();
        create_task(&conn, "Task 1", None, None, None, None).unwrap();
        create_task(&conn, "Task 2", None, None, None, None).unwrap();

        let new_orders = reindex_tasks(&conn).unwrap();

        assert_eq!(new_orders.len(), 2);
        assert_eq!(new_orders[0].1, 10.0);
        assert_eq!(new_orders[1].1, 20.0);
    }

    #[test]
    fn test_log_artifact() {
        let conn = setup();
        let task = create_task(&conn, "Task", None, Some("DoD"), None, None).unwrap();
        start_task(&conn, task.id).unwrap();

        let artifact = log_artifact(&conn, "research", ".tt/artifacts/1-research.md").unwrap();

        assert_eq!(artifact.name, "research");
        assert_eq!(artifact.file_path, ".tt/artifacts/1-research.md");
    }

    #[test]
    fn test_log_artifact_no_active_task() {
        let conn = setup();

        let result = log_artifact(&conn, "research", ".tt/artifacts/1-research.md");
        assert!(matches!(result.unwrap_err(), Error::NoActiveTask));
    }

    #[test]
    fn test_list_tasks_orders_by_dependencies() {
        let conn = setup();

        // Create tasks: A depends on B and C
        let task_c = create_task(&conn, "C", None, None, None, None).unwrap();
        let task_b = create_task(&conn, "B", None, None, None, None).unwrap();
        let task_a = create_task(&conn, "A", None, None, None, None).unwrap();

        // Set target to A
        set_target(&conn, task_a.id).unwrap();

        // Add dependencies: A depends on B, A depends on C
        add_dependency(&conn, task_a.id, task_b.id).unwrap();
        add_dependency(&conn, task_a.id, task_c.id).unwrap();

        // List tasks - B and C should come before A
        let tasks = list_tasks(&conn, false).unwrap();

        assert_eq!(tasks.len(), 3);
        // B and C should appear before A (dependencies first)
        let a_pos = tasks.iter().position(|t| t.id == task_a.id).unwrap();
        let b_pos = tasks.iter().position(|t| t.id == task_b.id).unwrap();
        let c_pos = tasks.iter().position(|t| t.id == task_c.id).unwrap();

        assert!(b_pos < a_pos, "B should come before A");
        assert!(c_pos < a_pos, "C should come before A");
    }
}

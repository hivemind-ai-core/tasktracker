//! High-level operations combining core logic with database
//!
//! This module provides the API that CLI and MCP layers use.
//! All operations enforce business rules and invariants.

use crate::core::models::{Artifact, Dependency, EditAction, Task, TaskDetail, TaskStatus};
use crate::core::{
    calculate_manual_order, can_block_task, can_cancel_task, can_complete_task, can_start_task,
    can_stop_task, can_unblock_task, find_next_task as find_next, get_blocked_tasks,
    is_target_reached, reindex_orders, would_create_cycle,
};
use crate::db;
use crate::error::{Error, Result};
use rusqlite::Connection;

/// Create a new task with automatic order calculation
/// Optionally add dependencies after creation
pub fn create_task(
    conn: &Connection,
    title: &str,
    description: &str,
    dod: &str,
    after_id: Option<i64>,
    before_id: Option<i64>,
    depends_on: Option<Vec<i64>>,
) -> Result<Task> {
    // Calculate manual order
    let all_tasks = db::tasks::get_all_tasks(conn)?;
    let manual_order = calculate_manual_order(&all_tasks, after_id, before_id)?;

    // Create the task
    let task = db::tasks::create_task(conn, title, description, dod, manual_order)?;

    // Add dependencies if specified
    if let Some(deps) = depends_on {
        if !deps.is_empty() {
            add_dependencies(conn, task.id, deps)?;
        }
    }

    Ok(task)
}

/// Edit task fields
pub fn edit_task(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    description: Option<&str>,
    dod: Option<&str>,
    status: Option<TaskStatus>,
    action: Option<EditAction>,
) -> Result<Task> {
    // Handle action if provided
    if let Some(act) = action {
        match act {
            EditAction::Complete => {
                complete_task(conn)?;
            }
            EditAction::Stop => {
                stop_task(conn)?;
            }
            EditAction::Cancel => {
                cancel_task(conn, id)?;
            }
            EditAction::Block => {
                block_task(conn, id)?;
            }
            EditAction::Unblock => {
                unblock_task(conn, id)?;
            }
        }
    }

    // Handle status change if requested
    if let Some(new_status) = status {
        let current_task = db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))?;

        // Only transition if status is actually changing
        if current_task.status != new_status {
            match new_status {
                TaskStatus::InProgress => {
                    start_task(conn, id)?;
                }
                TaskStatus::Pending => match current_task.status {
                    TaskStatus::InProgress => {
                        stop_task(conn)?;
                    }
                    TaskStatus::Blocked => {
                        unblock_task(conn, id)?;
                    }
                    _ => {}
                },
                TaskStatus::Completed => {
                    if current_task.status == TaskStatus::InProgress {
                        complete_task(conn)?;
                    } else {
                        return Err(Error::TaskNotPending(id));
                    }
                }
                TaskStatus::Blocked => {
                    block_task(conn, id)?;
                }
                TaskStatus::Cancelled => {
                    cancel_task(conn, id)?;
                }
            }
        }
    }

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
    db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))
}

/// Get multiple tasks by ID
/// Returns tasks that exist, ignores non-existent IDs
pub fn get_tasks(conn: &Connection, ids: Vec<i64>) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    for id in ids {
        if let Some(task) = db::tasks::get_task(conn, id, false)? {
            tasks.push(task);
        }
    }
    Ok(tasks)
}

/// Get multiple tasks by ID (includes archived tasks)
pub fn get_tasks_allow_archived(conn: &Connection, ids: Vec<i64>) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();
    for id in ids {
        if let Some(task) = db::tasks::get_task(conn, id, true)? {
            tasks.push(task);
        }
    }
    Ok(tasks)
}

/// Get task details with relationships
pub fn get_task_detail(conn: &Connection, id: i64) -> Result<TaskDetail> {
    let task = db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))?;

    // Get dependencies (prerequisites)
    let dep_records = db::dependencies::get_dependencies(conn, id)?;
    let mut dependencies = Vec::new();
    for dep in dep_records {
        if let Some(t) = db::tasks::get_task(conn, dep.depends_on, false)? {
            dependencies.push(t);
        }
    }

    // Get dependents (tasks that depend on this one)
    let dependent_records = db::dependencies::get_dependents(conn, id)?;
    let mut dependents = Vec::new();
    for dep in dependent_records {
        if let Some(t) = db::tasks::get_task(conn, dep.task_id, false)? {
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

/// Get task details with relationships (includes archived tasks)
pub fn get_task_detail_allow_archived(conn: &Connection, id: i64) -> Result<TaskDetail> {
    let task = db::tasks::get_task(conn, id, true)?.ok_or(Error::TaskNotFound(id))?;

    // Get dependencies (prerequisites)
    let dep_records = db::dependencies::get_dependencies(conn, id)?;
    let mut dependencies = Vec::new();
    for dep in dep_records {
        if let Some(t) = db::tasks::get_task(conn, dep.depends_on, true)? {
            dependencies.push(t);
        }
    }

    // Get dependents (tasks that depend on this one)
    let dependent_records = db::dependencies::get_dependents(conn, id)?;
    let mut dependents = Vec::new();
    for dep in dependent_records {
        if let Some(t) = db::tasks::get_task(conn, dep.task_id, true)? {
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
    let task = db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))?;

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
    db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))
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
    db::tasks::get_task(conn, task.id, false)?.ok_or(Error::TaskNotFound(task.id))
}

/// Complete the active task
pub fn complete_task(conn: &Connection) -> Result<Task> {
    // Get active task
    let task = db::tasks::get_active_task(conn)?.ok_or(Error::NoActiveTask)?;

    // Validate
    can_complete_task(&task)?;

    // Perform transition
    db::tasks::complete_task(conn, task.id)?;

    // Return updated task
    db::tasks::get_task(conn, task.id, false)?.ok_or(Error::TaskNotFound(task.id))
}

/// Cancel a specific task
pub fn cancel_task(conn: &Connection, id: i64) -> Result<Task> {
    let task = db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))?;

    // Validate
    can_cancel_task(&task)?;

    // Perform transition
    db::tasks::cancel_task(conn, id)?;

    // Return updated task
    db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))
}

/// Block a task
pub fn block_task(conn: &Connection, id: i64) -> Result<Task> {
    let task = db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))?;

    // Validate
    can_block_task(&task)?;

    // Perform transition
    db::tasks::block_task(conn, id)?;

    // Return updated task
    db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))
}

/// Unblock a task
pub fn unblock_task(conn: &Connection, id: i64) -> Result<Task> {
    let task = db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))?;

    // Validate
    can_unblock_task(&task)?;

    // Perform transition
    db::tasks::unblock_task(conn, id)?;

    // Return updated task
    db::tasks::get_task(conn, id, false)?.ok_or(Error::TaskNotFound(id))
}

/// Block multiple tasks
/// Returns a vector of results, one for each task ID
pub fn block_tasks(conn: &Connection, ids: Vec<i64>) -> Vec<Result<Task>> {
    let mut results = Vec::new();

    for id in ids {
        // If task is in progress, stop it first
        if let Ok(Some(task)) = db::tasks::get_task(conn, id, false) {
            if task.status == TaskStatus::InProgress {
                // Stop the task first
                let _ = db::tasks::stop_task(conn, id);
            }
        }

        let result = block_task(conn, id);
        results.push(result);
    }

    results
}

/// Unblock multiple tasks
/// Returns a vector of results, one for each task ID
pub fn unblock_tasks(conn: &Connection, ids: Vec<i64>) -> Vec<Result<Task>> {
    let mut results = Vec::new();

    for id in ids {
        let result = unblock_task(conn, id);
        results.push(result);
    }

    results
}

/// Add a dependency with cycle detection
pub fn add_dependency(conn: &Connection, task_id: i64, depends_on: i64) -> Result<()> {
    // Check if tasks exist
    if db::tasks::get_task(conn, task_id, false)?.is_none() {
        return Err(Error::TaskNotFound(task_id));
    }
    if db::tasks::get_task(conn, depends_on, false)?.is_none() {
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

/// Add multiple dependencies with validation and cycle detection
/// Stops on first error and returns that error
pub fn add_dependencies(conn: &Connection, task_id: i64, depends_on: Vec<i64>) -> Result<()> {
    // First, validate all task IDs exist
    for &dep_id in &depends_on {
        if db::tasks::get_task(conn, dep_id, false)?.is_none() {
            return Err(Error::TaskNotFound(dep_id));
        }
    }

    // Get all existing dependencies for cycle checking
    let all_deps = db::dependencies::get_all_dependencies(conn)?;

    // Check each dependency for cycles before adding any
    for &dep_id in &depends_on {
        if would_create_cycle(task_id, dep_id, &all_deps) {
            use crate::core::find_cycle_path;
            let cycle_path = find_cycle_path(task_id, dep_id, &all_deps)
                .unwrap_or_else(|| vec![task_id, dep_id, task_id]);
            return Err(Error::CycleDetected(task_id, dep_id, cycle_path));
        }
    }

    // Add all dependencies
    for dep_id in depends_on {
        db::dependencies::add_dependency(conn, task_id, dep_id)?;
    }

    Ok(())
}

/// Remove multiple dependencies
pub fn remove_dependencies(conn: &Connection, task_id: i64, depends_on: Vec<i64>) -> Result<()> {
    for dep_id in depends_on {
        db::dependencies::remove_dependency(conn, task_id, dep_id)?;
    }
    Ok(())
}

/// Set the target task
pub fn set_target(conn: &Connection, target_id: i64) -> Result<()> {
    // Verify task exists
    if db::tasks::get_task(conn, target_id, false)?.is_none() {
        return Err(Error::TaskNotFound(target_id));
    }

    db::config::set_target(conn, target_id)
}

/// Clear the target task
pub fn clear_target(conn: &Connection) -> Result<()> {
    db::config::clear_target(conn)
}

/// Get the current target
pub fn get_target(conn: &Connection) -> Result<Option<Task>> {
    match db::config::get_target(conn)? {
        Some(id) => db::tasks::get_task(conn, id, false),
        None => Ok(None),
    }
}

/// Get the next task to work on
/// If no target is set, finds the next globally runnable task
pub fn get_next_task(conn: &Connection) -> Result<Option<Task>> {
    // Check if target is set
    match db::config::get_target(conn)? {
        Some(target_id) => {
            // Target is set - use target-specific logic
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

            // Find next ready task in target subgraph
            Ok(find_next(target_id, &all_tasks, &all_deps))
        }
        None => {
            // No target set - use global next runnable
            find_next_runnable(conn)
        }
    }
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

/// List tasks with optional filtering and pagination
/// If all=true, returns all tasks regardless of target
/// If status is specified, filters by that status
/// If archived is Some(true), returns only archived tasks
/// If archived is Some(false) or None, returns non-archived tasks (default)
/// limit and offset control pagination
pub fn list_tasks(
    conn: &Connection,
    all: bool,
    status: Option<TaskStatus>,
    limit: Option<usize>,
    offset: Option<usize>,
    archived: Option<bool>,
) -> Result<Vec<Task>> {
    // Handle archived filter first
    if archived == Some(true) {
        return db::tasks::get_archived_tasks(conn, limit, offset);
    }

    // If status filter is specified, use it directly
    if let Some(s) = status {
        return db::tasks::get_tasks_by_status(conn, s, limit, offset);
    }

    if all {
        db::tasks::get_all_tasks_paginated(conn, limit, offset)
    } else {
        // Check if target is set
        match db::config::get_target(conn)? {
            Some(target_id) => {
                // Get subgraph
                let all_tasks = db::tasks::get_all_tasks(conn)?;
                let all_deps = db::dependencies::get_all_dependencies(conn)?;
                let subgraph =
                    crate::core::compute_target_subgraph(target_id, &all_tasks, &all_deps);

                // Sort topologically (dependency order), with manual_order as tiebreaker
                let sorted =
                    crate::core::topological_sort(subgraph, &all_deps).unwrap_or_else(|_| {
                        // Fallback to manual_order if there's a cycle
                        let mut sorted = all_tasks;
                        sorted.sort_by(|a, b| {
                            a.manual_order
                                .partial_cmp(&b.manual_order)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        sorted
                    });

                // Apply pagination
                let offset_val = offset.unwrap_or(0);
                let limited: Vec<Task> = sorted
                    .into_iter()
                    .skip(offset_val)
                    .take(limit.unwrap_or(usize::MAX))
                    .collect();

                Ok(limited)
            }
            None => {
                // No target set - return all tasks sorted by manual_order
                let mut tasks = db::tasks::get_all_tasks_paginated(conn, limit, offset)?;
                tasks.sort_by(|a, b| {
                    a.manual_order
                        .partial_cmp(&b.manual_order)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                Ok(tasks)
            }
        }
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

/// Log an artifact for a task (defaults to active task)
pub fn log_artifact(
    conn: &Connection,
    name: &str,
    file_path: &str,
    task_id: Option<i64>,
) -> Result<Artifact> {
    // Get task ID
    let id = match task_id {
        Some(id) => id,
        None => {
            db::tasks::get_active_task(conn)?
                .ok_or(Error::NoActiveTask)?
                .id
        }
    };

    // Create artifact
    db::artifacts::create_artifact(conn, id, name, file_path)
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

/// Archive all completed tasks
/// Returns the number of tasks archived
pub fn archive_completed(conn: &Connection) -> Result<usize> {
    db::tasks::archive_completed_tasks(conn)
}

/// Get the current active task
pub fn get_current_task(conn: &Connection) -> Result<Task> {
    db::tasks::get_active_task(conn)?.ok_or(Error::NoActiveTask)
}

/// Find the next runnable task globally (not target-specific)
/// Returns tasks with status=pending, not blocked, with all dependencies completed
/// Selects by lowest manual_order, then lowest id
pub fn find_next_runnable(conn: &Connection) -> Result<Option<Task>> {
    let all_tasks = db::tasks::get_all_tasks(conn)?;
    let all_deps = db::dependencies::get_all_dependencies(conn)?;

    // Filter to pending, non-blocked tasks with all deps completed
    let candidates: Vec<&Task> = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .filter(|t| {
            // Check all dependencies are completed
            let task_deps: Vec<&Dependency> =
                all_deps.iter().filter(|d| d.task_id == t.id).collect();
            task_deps.iter().all(|d| {
                all_tasks
                    .iter()
                    .find(|task| task.id == d.depends_on)
                    .map(|task| task.status == TaskStatus::Completed)
                    .unwrap_or(true) // If dep not found, consider it completed
            })
        })
        .collect();

    // Sort by manual_order, then by id
    let mut candidates: Vec<Task> = candidates.into_iter().cloned().collect();
    candidates.sort_by(|a, b| {
        a.manual_order
            .partial_cmp(&b.manual_order)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(candidates.into_iter().next())
}

/// Result of advancing to the next task
#[derive(Debug)]
pub struct AdvanceResult {
    pub completed: Option<Task>,
    pub started: Option<Task>,
}

/// Status summary for all tasks
#[derive(Debug)]
pub struct StatusSummary {
    pub pending: usize,
    pub blocked: usize,
    pub completed: usize,
    pub in_progress: usize,
    pub total: usize,
}

/// Full status output including target, current, next task and summary
#[derive(Debug)]
pub struct StatusOutput {
    pub target: Option<Task>,
    pub current_task: Option<Task>,
    pub next_task: Option<Task>,
    pub summary: StatusSummary,
}

/// Get comprehensive status of the task tracker
pub fn get_status(conn: &Connection) -> Result<StatusOutput> {
    let all_tasks = db::tasks::get_all_tasks(conn)?;

    // Count by status
    let pending = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .count();
    let blocked = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Blocked)
        .count();
    let completed = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Completed)
        .count();
    let in_progress = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::InProgress)
        .count();

    // Get target
    let target = get_target(conn)?;

    // Get current task
    let current_task = db::tasks::get_active_task(conn)?;

    // Get next task
    let next_task = find_next_runnable(conn)?;

    Ok(StatusOutput {
        target,
        current_task,
        next_task,
        summary: StatusSummary {
            pending,
            blocked,
            completed,
            in_progress,
            total: all_tasks.len(),
        },
    })
}

/// Advance workflow: complete current task and start the next one
/// If dry_run is true, returns preview without executing
pub fn advance_task(conn: &Connection, dry_run: bool) -> Result<AdvanceResult> {
    // Get current task
    let current = db::tasks::get_active_task(conn)?;

    // Find next runnable task
    let next = find_next_runnable(conn)?;

    if dry_run {
        return Ok(AdvanceResult {
            completed: current.clone(),
            started: next.clone(),
        });
    }

    // Complete current task if exists
    let completed = if let Some(ref _task) = current {
        // This will validate DoD is present
        let completed_task = complete_task(conn)?;
        Some(completed_task)
    } else {
        None
    };

    // Start next task if exists
    let started = if let Some(task) = next {
        Some(start_task(conn, task.id)?)
    } else {
        None
    };

    Ok(AdvanceResult { completed, started })
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
        let task = create_task(&conn, "Test Task", "", "", None, None, None).unwrap();

        assert_eq!(task.title, "Test Task");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_create_task_with_order() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", Some(task1.id), None, None).unwrap();

        assert!(task2.manual_order > task1.manual_order);
    }

    #[test]
    fn test_edit_task() {
        let conn = setup();
        let task = create_task(&conn, "Old Title", "", "", None, None, None).unwrap();

        let updated = edit_task(
            &conn,
            task.id,
            Some("New Title"),
            Some("Desc"),
            Some("DoD"),
            None,
            None,
        )
        .unwrap();

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.description, Some("Desc".to_string()));
        assert_eq!(updated.dod, Some("DoD".to_string()));
    }

    #[test]
    fn test_get_task_detail() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", None, None, None).unwrap();

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
        let task = create_task(&conn, "Task", "", "DoD", None, None, None).unwrap();

        let started = start_task(&conn, task.id).unwrap();
        assert_eq!(started.status, TaskStatus::InProgress);

        let completed = complete_task(&conn).unwrap();
        assert_eq!(completed.status, TaskStatus::Completed);
    }

    #[test]
    fn test_cannot_complete_without_dod() {
        let conn = setup();
        let task = create_task(&conn, "Task", "", "", None, None, None).unwrap();
        start_task(&conn, task.id).unwrap();

        let result = complete_task(&conn);
        assert!(matches!(result.unwrap_err(), Error::NoDod(_)));
    }

    #[test]
    fn test_cannot_start_with_unmet_deps() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", None, None, None).unwrap();

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
        let task1 = create_task(&conn, "Task 1", "", "", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", None, None, None).unwrap();
        let task3 = create_task(&conn, "Task 3", "", "", None, None, None).unwrap();

        add_dependency(&conn, task2.id, task1.id).unwrap();
        add_dependency(&conn, task3.id, task2.id).unwrap();

        // Adding 1 -> 3 would create a cycle
        let result = add_dependency(&conn, task1.id, task3.id);
        assert!(matches!(result.unwrap_err(), Error::CycleDetected(_, _, _)));
    }

    #[test]
    fn test_target_operations() {
        let conn = setup();
        let task = create_task(&conn, "Target", "", "", None, None, None).unwrap();

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
        let task = create_task(&conn, "Target", "", "DoD", None, None, None).unwrap();

        set_target(&conn, task.id).unwrap();
        start_task(&conn, task.id).unwrap();
        complete_task(&conn).unwrap();

        let result = get_next_task(&conn);
        assert!(matches!(result.unwrap_err(), Error::TargetReached(_)));
    }

    #[test]
    fn test_reorder_task() {
        let conn = setup();
        let task1 = create_task(&conn, "Task 1", "", "", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "", None, None, None).unwrap();

        // Reorder task2 to be before task1
        let new_order = reorder_task(&conn, task2.id, None, Some(task1.id)).unwrap();

        let updated = db::tasks::get_task(&conn, task2.id, false)
            .unwrap()
            .unwrap();
        assert_eq!(updated.manual_order, new_order);
        assert!(updated.manual_order < task1.manual_order);
    }

    #[test]
    fn test_reindex_tasks() {
        let conn = setup();
        create_task(&conn, "Task 1", "", "", None, None, None).unwrap();
        create_task(&conn, "Task 2", "", "", None, None, None).unwrap();

        let new_orders = reindex_tasks(&conn).unwrap();

        assert_eq!(new_orders.len(), 2);
        assert_eq!(new_orders[0].1, 10.0);
        assert_eq!(new_orders[1].1, 20.0);
    }

    #[test]
    fn test_log_artifact() {
        let conn = setup();
        let task = create_task(&conn, "Task", "", "DoD", None, None, None).unwrap();
        start_task(&conn, task.id).unwrap();

        let artifact =
            log_artifact(&conn, "research", ".tt/artifacts/1-research.md", None).unwrap();

        assert_eq!(artifact.name, "research");
        assert_eq!(artifact.file_path, ".tt/artifacts/1-research.md");
    }

    #[test]
    fn test_log_artifact_no_active_task() {
        let conn = setup();

        let result = log_artifact(&conn, "research", ".tt/artifacts/1-research.md", None);
        assert!(matches!(result.unwrap_err(), Error::NoActiveTask));
    }

    #[test]
    fn test_list_tasks_orders_by_dependencies() {
        let conn = setup();

        // Create tasks: A depends on B and C
        let task_c = create_task(&conn, "C", "", "", None, None, None).unwrap();
        let task_b = create_task(&conn, "B", "", "", None, None, None).unwrap();
        let task_a = create_task(&conn, "A", "", "", None, None, None).unwrap();

        // Set target to A
        set_target(&conn, task_a.id).unwrap();

        // Add dependencies: A depends on B, A depends on C
        add_dependency(&conn, task_a.id, task_b.id).unwrap();
        add_dependency(&conn, task_a.id, task_c.id).unwrap();

        // List tasks - B and C should come before A
        let tasks = list_tasks(&conn, false, None, None, None, None).unwrap();

        assert_eq!(tasks.len(), 3);
        // B and C should appear before A (dependencies first)
        let a_pos = tasks.iter().position(|t| t.id == task_a.id).unwrap();
        let b_pos = tasks.iter().position(|t| t.id == task_b.id).unwrap();
        let c_pos = tasks.iter().position(|t| t.id == task_c.id).unwrap();

        assert!(b_pos < a_pos, "B should come before A");
        assert!(c_pos < a_pos, "C should come before A");
    }

    #[test]
    fn test_split_task() {
        let conn = setup();

        // Create original task
        let original = create_task(&conn, "Original", "Desc", "DoD", None, None, None).unwrap();

        // Create a task that depends on the original
        let dependent = create_task(&conn, "Dependent", "", "", None, None, None).unwrap();
        add_dependency(&conn, dependent.id, original.id).unwrap();

        // Split the original into two subtasks
        let subtasks = vec![
            (
                "Subtask 1".to_string(),
                "Desc 1".to_string(),
                "DoD 1".to_string(),
            ),
            (
                "Subtask 2".to_string(),
                "Desc 2".to_string(),
                "DoD 2".to_string(),
            ),
        ];
        let new_tasks = split_task(&conn, original.id, subtasks).unwrap();

        assert_eq!(new_tasks.len(), 2);

        // Original should be soft-deleted
        assert!(db::tasks::get_task(&conn, original.id, false)
            .unwrap()
            .is_none());

        // New tasks should exist
        let subtask1 = db::tasks::get_task(&conn, new_tasks[0].id, false)
            .unwrap()
            .unwrap();
        let subtask2 = db::tasks::get_task(&conn, new_tasks[1].id, false)
            .unwrap()
            .unwrap();
        assert_eq!(subtask1.title, "Subtask 1");
        assert_eq!(subtask2.title, "Subtask 2");

        // Dependent should now depend on both subtasks
        let deps = db::dependencies::get_dependencies(&conn, dependent.id).unwrap();
        assert_eq!(deps.len(), 2);
        let dep_ids: Vec<i64> = deps.iter().map(|d| d.depends_on).collect();
        assert!(dep_ids.contains(&subtask1.id));
        assert!(dep_ids.contains(&subtask2.id));
    }

    #[test]
    fn test_split_task_preserves_original_deps() {
        let conn = setup();

        // Create: Dep -> Original -> Prereq
        let prereq = create_task(&conn, "Prereq", "", "", None, None, None).unwrap();
        let original = create_task(&conn, "Original", "", "", None, None, None).unwrap();
        add_dependency(&conn, original.id, prereq.id).unwrap();

        // Split original
        let subtasks = vec![
            ("Subtask 1".to_string(), "".to_string(), "".to_string()),
            ("Subtask 2".to_string(), "".to_string(), "".to_string()),
        ];
        let new_tasks = split_task(&conn, original.id, subtasks).unwrap();

        // Subtasks should inherit dependency on prereq
        for task in &new_tasks {
            let deps = db::dependencies::get_dependencies(&conn, task.id).unwrap();
            assert_eq!(deps.len(), 1);
            assert_eq!(deps[0].depends_on, prereq.id);
        }
    }

    #[test]
    fn test_advance_task_atomic() {
        let conn = setup();

        // Create two tasks
        let task1 = create_task(&conn, "Task 1", "", "DoD", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "DoD", None, None, None).unwrap();

        // Start first task
        start_task(&conn, task1.id).unwrap();

        // Verify only one task is in_progress
        let in_progress: Vec<_> = db::tasks::get_all_tasks(&conn)
            .unwrap()
            .into_iter()
            .filter(|t| t.status == TaskStatus::InProgress)
            .collect();
        assert_eq!(in_progress.len(), 1);

        // Advance - should complete task1 and start task2
        let result = advance_task(&conn, false).unwrap();

        assert!(result.completed.is_some());
        assert!(result.started.is_some());
        assert_eq!(result.completed.unwrap().id, task1.id);
        assert_eq!(result.started.unwrap().id, task2.id);

        // Verify only one task is still in_progress (task2)
        let in_progress: Vec<_> = db::tasks::get_all_tasks(&conn)
            .unwrap()
            .into_iter()
            .filter(|t| t.status == TaskStatus::InProgress)
            .collect();
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].id, task2.id);

        // Verify task1 is now completed
        let task1_updated = db::tasks::get_task(&conn, task1.id, false)
            .unwrap()
            .unwrap();
        assert_eq!(task1_updated.status, TaskStatus::Completed);
    }

    #[test]
    fn test_archive_completed() {
        let conn = setup();

        // Create and complete tasks
        let task1 = create_task(&conn, "Task 1", "", "DoD", None, None, None).unwrap();
        let task2 = create_task(&conn, "Task 2", "", "DoD", None, None, None).unwrap();
        let task3 = create_task(&conn, "Task 3", "", "DoD", None, None, None).unwrap();

        // Complete task1 and task2
        start_task(&conn, task1.id).unwrap();
        complete_task(&conn).unwrap();
        start_task(&conn, task2.id).unwrap();
        complete_task(&conn).unwrap();

        // Verify all tasks exist
        let all = db::tasks::get_all_tasks(&conn).unwrap();
        assert_eq!(all.len(), 3);

        // Archive completed tasks
        let count = archive_completed(&conn).unwrap();
        assert_eq!(count, 2);

        // Verify archived tasks are gone from normal list
        let remaining = db::tasks::get_all_tasks(&conn).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, task3.id);

        // Verify archived tasks exist in archived list
        let archived = db::tasks::get_archived_tasks(&conn, None, None).unwrap();
        assert_eq!(archived.len(), 2);
    }

    #[test]
    fn test_list_tasks_excludes_archived_by_default() {
        let conn = setup();

        // Create tasks
        let task1 = create_task(&conn, "Active", "", "DoD", None, None, None).unwrap();
        let task2 = create_task(&conn, "Completed", "", "DoD", None, None, None).unwrap();

        // Complete task2 and archive it
        start_task(&conn, task2.id).unwrap();
        complete_task(&conn).unwrap();
        archive_completed(&conn).unwrap();

        // Default list should exclude archived
        let tasks = list_tasks(&conn, true, None, None, None, None).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task1.id);

        // List with archived=true should only show archived
        let archived = list_tasks(&conn, true, None, None, None, Some(true)).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, task2.id);
    }
}

/// Split a task into multiple subtasks
///
/// The original task is soft-deleted. All dependents of the original
/// will now depend on ALL of the new subtasks (AND relationship).
/// All dependencies of the original are inherited by each subtask.
///
/// Returns the newly created tasks.
pub fn split_task(
    conn: &Connection,
    task_id: i64,
    subtasks: Vec<(String, String, String)>,
) -> Result<Vec<Task>> {
    // Verify original task exists and is not deleted
    let original =
        db::tasks::get_task(conn, task_id, false)?.ok_or(Error::TaskNotFound(task_id))?;

    // Get original dependencies (what original depended on)
    let original_deps = db::dependencies::get_dependencies(conn, task_id)?;
    let prereq_ids: Vec<i64> = original_deps.iter().map(|d| d.depends_on).collect();

    // Get original dependents (what depended on original)
    let original_dependents = db::dependencies::get_dependents(conn, task_id)?;
    let dependent_ids: Vec<i64> = original_dependents.iter().map(|d| d.task_id).collect();

    // Create new subtasks at the original task's position
    let mut new_tasks = Vec::new();
    for (title, desc, dod) in subtasks {
        let task = db::tasks::create_task(conn, &title, &desc, &dod, original.manual_order)?;
        new_tasks.push(task);
    }

    // Add original dependencies to all subtasks
    for subtask in &new_tasks {
        for &prereq_id in &prereq_ids {
            db::dependencies::add_dependency(conn, subtask.id, prereq_id)?;
        }
    }

    // Update all dependents to depend on new subtasks instead of original
    for dependent_id in dependent_ids {
        // Remove dependency on original
        db::dependencies::remove_dependency(conn, dependent_id, task_id)?;
        // Add dependencies on all new subtasks
        for subtask in &new_tasks {
            db::dependencies::add_dependency(conn, dependent_id, subtask.id)?;
        }
    }

    // Soft-delete the original task
    db::tasks::soft_delete_task(conn, task_id)?;

    Ok(new_tasks)
}

//! State machine for task status transitions
//!
//! Implements all valid transitions per SPEC.md Section 6.

use crate::core::models::{Task, TaskStatus};
use crate::error::{Error, Result};

/// Validate a state transition from one status to another
///
/// Returns Ok(()) if the transition is valid, or an appropriate error.
pub fn validate_transition(from: TaskStatus, to: TaskStatus, task_id: i64) -> Result<()> {
    match (from, to) {
        // Valid transitions
        (TaskStatus::Pending, TaskStatus::InProgress) => Ok(()),
        (TaskStatus::InProgress, TaskStatus::Pending) => Ok(()),
        (TaskStatus::InProgress, TaskStatus::Completed) => Ok(()),
        (TaskStatus::Pending, TaskStatus::Blocked) => Ok(()),
        (TaskStatus::InProgress, TaskStatus::Blocked) => Ok(()),
        (TaskStatus::Blocked, TaskStatus::Pending) => Ok(()),
        // Cancelled state transitions
        (TaskStatus::Pending, TaskStatus::Cancelled) => Ok(()),
        (TaskStatus::InProgress, TaskStatus::Cancelled) => Ok(()),

        // Idempotent: starting an already in-progress task
        (TaskStatus::InProgress, TaskStatus::InProgress) => Ok(()),

        // Invalid transitions
        (TaskStatus::Pending, TaskStatus::Completed) => Err(Error::TaskNotPending(task_id)),
        (TaskStatus::Blocked, TaskStatus::InProgress) => Err(Error::TaskNotPending(task_id)),
        (TaskStatus::Blocked, TaskStatus::Completed) => Err(Error::TaskNotPending(task_id)),
        (TaskStatus::Completed, _) => Err(Error::NotSupported(
            "Completed tasks cannot change status".to_string(),
        )),
        (TaskStatus::Cancelled, _) => Err(Error::NotSupported(
            "Cancelled tasks cannot change status".to_string(),
        )),
        (from, to) => Err(Error::NotSupported(format!(
            "Cannot transition from {from:?} to {to:?}"
        ))),
    }
}

/// Check if a task can be started
///
/// Guards:
/// - Task must be pending
/// - No other task can be in progress
/// - All dependencies must be completed
pub fn can_start_task(
    task: &Task,
    active_task: Option<&Task>,
    incomplete_deps: &[i64],
) -> Result<()> {
    // Check if already in progress (idempotent)
    if task.status == TaskStatus::InProgress {
        return Ok(());
    }

    // Task must be pending
    if task.status != TaskStatus::Pending {
        return Err(Error::TaskNotPending(task.id));
    }

    // No other task can be active
    if let Some(active) = active_task {
        return Err(Error::AnotherTaskActive(active.id, active.title.clone()));
    }

    // All dependencies must be completed
    if !incomplete_deps.is_empty() {
        return Err(Error::UnmetDependencies(task.id, incomplete_deps.to_vec()));
    }

    Ok(())
}

/// Check if a task can be completed
///
/// Guards:
/// - Task must be in progress
/// - Task must have a definition of done
pub fn can_complete_task(task: &Task) -> Result<()> {
    // Task must be in progress
    if task.status != TaskStatus::InProgress {
        return Err(Error::NoActiveTask);
    }

    // Must have DoD
    if task
        .dod
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true)
    {
        return Err(Error::NoDod(task.id));
    }

    Ok(())
}

/// Check if a task can be stopped
///
/// Guard: Task must be in progress
pub fn can_stop_task(task: &Task) -> Result<()> {
    if task.status != TaskStatus::InProgress {
        return Err(Error::NoActiveTask);
    }
    Ok(())
}

/// Check if a task can be blocked
///
/// Guards:
/// - Task must be pending or in_progress
pub fn can_block_task(task: &Task) -> Result<()> {
    if !task.status.can_block() {
        return Err(Error::NotSupported(format!(
            "Cannot block task with status {:?}",
            task.status
        )));
    }
    Ok(())
}

/// Check if a task can be unblocked
///
/// Guard: Task must be blocked
pub fn can_unblock_task(task: &Task) -> Result<()> {
    if !task.status.can_unblock() {
        return Err(Error::NotSupported(format!(
            "Cannot unblock task with status {:?}",
            task.status
        )));
    }
    Ok(())
}

/// Check if a task can be cancelled
///
/// Guards:
/// - Task must be pending or in_progress
pub fn can_cancel_task(task: &Task) -> Result<()> {
    if !task.status.can_cancel() {
        return Err(Error::NotSupported(format!(
            "Cannot cancel task with status {:?}",
            task.status
        )));
    }
    Ok(())
}

/// Get the new status after a transition
///
/// This is mostly for documentation, as the status is explicitly passed.
pub fn transition_status(_current: TaskStatus, target: TaskStatus) -> TaskStatus {
    target
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task(id: i64, status: TaskStatus) -> Task {
        Task {
            id,
            title: format!("Task {id}"),
            description: None,
            dod: None,
            status,
            manual_order: 10.0,
            created_at: "2025-06-01T10:00:00".to_string(),
            started_at: None,
            completed_at: None,
            last_touched_at: "2025-06-01T10:00:00".to_string(),
            deleted: false,
        }
    }

    fn make_task_with_dod(id: i64, status: TaskStatus, dod: &str) -> Task {
        let mut task = make_task(id, status);
        task.dod = Some(dod.to_string());
        task
    }

    // --- validate_transition tests ---

    #[test]
    fn test_valid_transitions() {
        // Pending -> InProgress
        assert!(validate_transition(TaskStatus::Pending, TaskStatus::InProgress, 1).is_ok());
        // InProgress -> Pending
        assert!(validate_transition(TaskStatus::InProgress, TaskStatus::Pending, 1).is_ok());
        // InProgress -> Completed
        assert!(validate_transition(TaskStatus::InProgress, TaskStatus::Completed, 1).is_ok());
        // Pending -> Blocked
        assert!(validate_transition(TaskStatus::Pending, TaskStatus::Blocked, 1).is_ok());
        // InProgress -> Blocked
        assert!(validate_transition(TaskStatus::InProgress, TaskStatus::Blocked, 1).is_ok());
        // Blocked -> Pending
        assert!(validate_transition(TaskStatus::Blocked, TaskStatus::Pending, 1).is_ok());
    }

    #[test]
    fn test_idempotent_start() {
        // InProgress -> InProgress (already active)
        assert!(validate_transition(TaskStatus::InProgress, TaskStatus::InProgress, 1).is_ok());
    }

    #[test]
    fn test_invalid_pending_to_completed() {
        let result = validate_transition(TaskStatus::Pending, TaskStatus::Completed, 5);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::TaskNotPending(5)));
    }

    #[test]
    fn test_invalid_blocked_to_in_progress() {
        let result = validate_transition(TaskStatus::Blocked, TaskStatus::InProgress, 7);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_completed_transitions() {
        // Completed cannot transition to anything
        let result = validate_transition(TaskStatus::Completed, TaskStatus::Pending, 1);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotSupported(_)));
    }

    #[test]
    fn test_valid_pending_to_cancelled() {
        assert!(validate_transition(TaskStatus::Pending, TaskStatus::Cancelled, 1).is_ok());
    }

    #[test]
    fn test_valid_in_progress_to_cancelled() {
        assert!(validate_transition(TaskStatus::InProgress, TaskStatus::Cancelled, 1).is_ok());
    }

    #[test]
    fn test_invalid_cancelled_transitions() {
        // Cancelled cannot transition to anything
        let result = validate_transition(TaskStatus::Cancelled, TaskStatus::Pending, 1);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NotSupported(_)));
    }

    // --- can_start_task tests ---

    #[test]
    fn test_can_start_pending_task() {
        let task = make_task(1, TaskStatus::Pending);
        let result = can_start_task(&task, None, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_start_already_in_progress() {
        let task = make_task(1, TaskStatus::InProgress);
        let result = can_start_task(&task, None, &[]);
        assert!(result.is_ok()); // Idempotent
    }

    #[test]
    fn test_cannot_start_blocked_task() {
        let task = make_task(1, TaskStatus::Blocked);
        let result = can_start_task(&task, None, &[]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::TaskNotPending(1)));
    }

    #[test]
    fn test_cannot_start_with_active_task() {
        let task = make_task(1, TaskStatus::Pending);
        let active = make_task(2, TaskStatus::InProgress);
        let result = can_start_task(&task, Some(&active), &[]);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::AnotherTaskActive(2, _)
        ));
    }

    #[test]
    fn test_cannot_start_with_unmet_dependencies() {
        let task = make_task(1, TaskStatus::Pending);
        let result = can_start_task(&task, None, &[5, 6]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            Error::UnmetDependencies(id, deps) => {
                assert_eq!(id, 1);
                assert_eq!(deps, vec![5, 6]);
            }
            _ => panic!("Expected UnmetDependencies error"),
        }
    }

    // --- can_complete_task tests ---

    #[test]
    fn test_can_complete_in_progress_with_dod() {
        let task = make_task_with_dod(1, TaskStatus::InProgress, "Definition of done");
        let result = can_complete_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cannot_complete_pending_task() {
        let task = make_task_with_dod(1, TaskStatus::Pending, "Definition of done");
        let result = can_complete_task(&task);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NoActiveTask));
    }

    #[test]
    fn test_cannot_complete_without_dod() {
        let task = make_task(1, TaskStatus::InProgress);
        let result = can_complete_task(&task);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NoDod(1)));
    }

    #[test]
    fn test_cannot_complete_with_empty_dod() {
        let task = make_task_with_dod(1, TaskStatus::InProgress, "   ");
        let result = can_complete_task(&task);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NoDod(1)));
    }

    // --- can_stop_task tests ---

    #[test]
    fn test_can_stop_in_progress_task() {
        let task = make_task(1, TaskStatus::InProgress);
        let result = can_stop_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cannot_stop_pending_task() {
        let task = make_task(1, TaskStatus::Pending);
        let result = can_stop_task(&task);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::NoActiveTask));
    }

    // --- can_block_task tests ---

    #[test]
    fn test_can_block_pending_task() {
        let task = make_task(1, TaskStatus::Pending);
        let result = can_block_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_block_in_progress_task() {
        let task = make_task(1, TaskStatus::InProgress);
        let result = can_block_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cannot_block_blocked_task() {
        let task = make_task(1, TaskStatus::Blocked);
        let result = can_block_task(&task);
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_block_completed_task() {
        let task = make_task(1, TaskStatus::Completed);
        let result = can_block_task(&task);
        assert!(result.is_err());
    }

    // --- can_unblock_task tests ---

    #[test]
    fn test_can_unblock_blocked_task() {
        let task = make_task(1, TaskStatus::Blocked);
        let result = can_unblock_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cannot_unblock_pending_task() {
        let task = make_task(1, TaskStatus::Pending);
        let result = can_unblock_task(&task);
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_unblock_in_progress_task() {
        let task = make_task(1, TaskStatus::InProgress);
        let result = can_unblock_task(&task);
        assert!(result.is_err());
    }

    // --- can_cancel_task tests ---

    #[test]
    fn test_can_cancel_pending_task() {
        let task = make_task(1, TaskStatus::Pending);
        let result = can_cancel_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_cancel_in_progress_task() {
        let task = make_task(1, TaskStatus::InProgress);
        let result = can_cancel_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_cancel_blocked_task() {
        let task = make_task(1, TaskStatus::Blocked);
        let result = can_cancel_task(&task);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cannot_cancel_completed_task() {
        let task = make_task(1, TaskStatus::Completed);
        let result = can_cancel_task(&task);
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_cancel_cancelled_task() {
        let task = make_task(1, TaskStatus::Cancelled);
        let result = can_cancel_task(&task);
        assert!(result.is_err());
    }
}

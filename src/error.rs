//! Error types for the tt task tracker
//!
//! All error variants from SPEC.md Section 15

use thiserror::Error;

/// Main error type for the tt application
#[derive(Error, Debug)]
pub enum Error {
    /// Task not found by ID
    #[error("Task #{0} not found")]
    TaskNotFound(i64),

    /// Task is not in pending status, cannot start
    #[error("Task #{0} is not pending, cannot start")]
    TaskNotPending(i64),

    /// Another task is already in progress
    #[error("Task #{0} is already in progress. Finish or stop it first.")]
    AnotherTaskActive(i64),

    /// No task is currently active
    #[error("No task is currently in progress")]
    NoActiveTask,

    /// Task has unmet dependencies
    #[error("Cannot start #{0}: dependencies not completed: {1:?}")]
    UnmetDependencies(i64, Vec<i64>),

    /// Adding dependency would create a cycle
    #[error("Adding #{0} -> #{1} would create a cycle: {2:?}")]
    CycleDetected(i64, i64, Vec<i64>),

    /// No target is set
    #[error("No target set. Use `tt target <id>` first.")]
    NoTarget,

    /// Target has been reached (all tasks completed)
    #[error("Target reached. All tasks for #{0} are completed.")]
    TargetReached(i64),

    /// Task has no definition of done
    #[error("Task #{0} has no definition of done. Set one with `tt edit {0} --dod`")]
    NoDod(i64),

    /// Manual order conflict warning (not a hard error)
    #[error("Warning: #{0} (order {1}) depends on #{2} (order {3}) which has higher manual_order")]
    OrderConflict(i64, f64, i64, f64),

    /// Invalid status string
    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    /// All remaining tasks are blocked
    #[error("All remaining tasks are blocked: {0:?}")]
    AllBlocked(Vec<i64>),

    /// Database error from rusqlite
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON-RPC error for MCP
    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),

    /// Operation not supported
    #[error("Not supported: {0}")]
    NotSupported(String),

    /// Float precision exhausted for manual ordering
    #[error("Float precision exhausted. Run `tt reindex` to reset ordering.")]
    FloatPrecisionExhausted,

    /// MCP error
    #[error("MCP error: {0}")]
    Mcp(String),

    /// Invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_task_not_found() {
        let err = Error::TaskNotFound(42);
        assert_eq!(err.to_string(), "Task #42 not found");
    }

    #[test]
    fn test_error_another_task_active() {
        let err = Error::AnotherTaskActive(5);
        assert_eq!(
            err.to_string(),
            "Task #5 is already in progress. Finish or stop it first."
        );
    }

    #[test]
    fn test_error_cycle_detected() {
        let err = Error::CycleDetected(1, 3, vec![1, 2, 3, 1]);
        assert_eq!(
            err.to_string(),
            "Adding #1 -> #3 would create a cycle: [1, 2, 3, 1]"
        );
    }

    #[test]
    fn test_error_unmet_dependencies() {
        let err = Error::UnmetDependencies(10, vec![5, 6]);
        assert_eq!(
            err.to_string(),
            "Cannot start #10: dependencies not completed: [5, 6]"
        );
    }

    #[test]
    fn test_error_no_dod() {
        let err = Error::NoDod(7);
        assert_eq!(
            err.to_string(),
            "Task #7 has no definition of done. Set one with `tt edit 7 --dod`"
        );
    }

    #[test]
    fn test_error_float_precision_exhausted() {
        let err = Error::FloatPrecisionExhausted;
        assert_eq!(
            err.to_string(),
            "Float precision exhausted. Run `tt reindex` to reset ordering."
        );
    }
}

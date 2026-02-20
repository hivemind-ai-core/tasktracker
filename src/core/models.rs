//! Core data models for tt
//!
//! Defines Task, TaskStatus, Dependency, Artifact, and TaskDetail structs.

use serde::{Deserialize, Serialize};

/// Task status with strict state transitions per SPEC.md Section 6
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is waiting to be started
    Pending,
    /// Task is currently being worked on
    InProgress,
    /// Task has been completed
    Completed,
    /// Task is blocked and cannot be started
    Blocked,
}

impl TaskStatus {
    /// Convert from database string
    pub fn from_db(s: &str) -> crate::error::Result<Self> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "blocked" => Ok(TaskStatus::Blocked),
            _ => Err(crate::error::Error::InvalidStatus(s.to_string())),
        }
    }

    /// Convert to database string
    pub fn to_db(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Blocked => "blocked",
        }
    }

    /// Check if this status allows starting the task
    pub fn can_start(&self) -> bool {
        matches!(self, TaskStatus::Pending)
    }

    /// Check if this status allows blocking
    pub fn can_block(&self) -> bool {
        matches!(self, TaskStatus::Pending | TaskStatus::InProgress)
    }

    /// Check if this status allows unblocking
    pub fn can_unblock(&self) -> bool {
        matches!(self, TaskStatus::Blocked)
    }

    /// Get display character for CLI
    pub fn display_char(&self) -> char {
        match self {
            TaskStatus::Pending => '○',
            TaskStatus::InProgress => '●',
            TaskStatus::Completed => '✓',
            TaskStatus::Blocked => '✗',
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_db())
    }
}

/// Represents a task row from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub dod: Option<String>,
    pub status: TaskStatus,
    pub manual_order: f64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

/// A dependency edge: task_id depends on depends_on
#[derive(Debug, Clone)]
pub struct Dependency {
    pub task_id: i64,
    pub depends_on: i64,
}

/// An artifact linked to a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_id: i64,
    pub name: String,
    pub file_path: String,
    pub created_at: String,
}

/// A task with its relationships populated
#[derive(Debug, Clone, Serialize)]
pub struct TaskDetail {
    pub task: Task,
    pub dependencies: Vec<Task>,
    pub dependents: Vec<Task>,
    pub artifacts: Vec<Artifact>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_from_db() {
        assert_eq!(TaskStatus::from_db("pending").unwrap(), TaskStatus::Pending);
        assert_eq!(
            TaskStatus::from_db("in_progress").unwrap(),
            TaskStatus::InProgress
        );
        assert_eq!(
            TaskStatus::from_db("completed").unwrap(),
            TaskStatus::Completed
        );
        assert_eq!(TaskStatus::from_db("blocked").unwrap(), TaskStatus::Blocked);
        assert!(TaskStatus::from_db("invalid").is_err());
    }

    #[test]
    fn test_task_status_to_db() {
        assert_eq!(TaskStatus::Pending.to_db(), "pending");
        assert_eq!(TaskStatus::InProgress.to_db(), "in_progress");
        assert_eq!(TaskStatus::Completed.to_db(), "completed");
        assert_eq!(TaskStatus::Blocked.to_db(), "blocked");
    }

    #[test]
    fn test_task_status_can_start() {
        assert!(TaskStatus::Pending.can_start());
        assert!(!TaskStatus::InProgress.can_start());
        assert!(!TaskStatus::Completed.can_start());
        assert!(!TaskStatus::Blocked.can_start());
    }

    #[test]
    fn test_task_status_can_block() {
        assert!(TaskStatus::Pending.can_block());
        assert!(TaskStatus::InProgress.can_block());
        assert!(!TaskStatus::Completed.can_block());
        assert!(!TaskStatus::Blocked.can_block());
    }

    #[test]
    fn test_task_status_can_unblock() {
        assert!(!TaskStatus::Pending.can_unblock());
        assert!(!TaskStatus::InProgress.can_unblock());
        assert!(!TaskStatus::Completed.can_unblock());
        assert!(TaskStatus::Blocked.can_unblock());
    }

    #[test]
    fn test_task_status_display_char() {
        assert_eq!(TaskStatus::Pending.display_char(), '○');
        assert_eq!(TaskStatus::InProgress.display_char(), '●');
        assert_eq!(TaskStatus::Completed.display_char(), '✓');
        assert_eq!(TaskStatus::Blocked.display_char(), '✗');
    }

    #[test]
    fn test_task_serialization() {
        let task = Task {
            id: 1,
            title: "Test Task".to_string(),
            description: Some("Description".to_string()),
            dod: Some("DoD".to_string()),
            status: TaskStatus::Pending,
            manual_order: 10.0,
            created_at: "2025-06-01T10:00:00".to_string(),
            started_at: None,
            completed_at: None,
            last_touched_at: "2025-06-01T10:00:00".to_string(),
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("Test Task"));
        assert!(json.contains("pending"));
    }
}

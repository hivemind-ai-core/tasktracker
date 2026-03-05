//! MCP tool handlers
//!
//! Implements all 16 MCP tools as defined in SPEC.md Section 13.2

use crate::core::{
    add_dependencies, add_dependency, advance_task, archive_completed, block_task, block_tasks,
    clear_target, create_task, edit_task, get_current_task, get_target, get_task_artifacts,
    get_task_detail_allow_archived, get_tasks_allow_archived, list_tasks, log_artifact,
    remove_dependencies, remove_dependency, reorder_task, set_target, split_task, unblock_task,
    unblock_tasks,
};
use crate::mcp::transport::McpResponse;
use rusqlite::Connection;
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;

/// Tool metadata with JSON Schema
#[derive(Debug, Clone)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

impl ToolMetadata {
    /// Create new tool metadata
    pub fn new(name: &str, description: &str, input_schema: serde_json::Value) -> Self {
        ToolMetadata {
            name: name.to_string(),
            description: description.to_string(),
            input_schema,
        }
    }
}

/// Tool handler trait
pub trait ToolHandler: Send + Sync {
    /// Handle a tool call
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String>;

    /// Get tool metadata
    fn metadata(&self) -> ToolMetadata;
}

/// Handler registry type
pub type HandlerRegistry = HashMap<String, Box<dyn ToolHandler>>;

// ============================================================================
// Tool Input Schemas (using schemars for JSON Schema generation)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
struct GetTaskInput {
    id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListTasksInput {
    #[serde(default)]
    no_focus: bool,
    #[serde(default)]
    archived: Option<bool>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    ids: Option<Vec<i64>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateTaskInput {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    dod: Option<String>,
    #[serde(default)]
    after_id: Option<i64>,
    #[serde(default)]
    before_id: Option<i64>,
    #[serde(default)]
    depends_on: Vec<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EditTaskInput {
    id: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    dod: Option<String>,
    /// Set task status (pending, in_progress, completed, blocked, cancelled)
    #[serde(default)]
    status: Option<String>,
    /// Action to perform (complete, stop, cancel, block, unblock)
    #[serde(default)]
    action: Option<String>,
    /// Add dependencies (task will depend on these IDs)
    #[serde(default)]
    depends_on: Option<Vec<i64>>,
    /// Remove dependencies
    #[serde(default)]
    remove_depends_on: Option<Vec<i64>>,
    /// Move task after this ID
    #[serde(default)]
    after: Option<i64>,
    /// Move task before this ID
    #[serde(default)]
    before: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TaskIdInput {
    id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DependencyInput {
    task_id: i64,
    depends_on: Vec<i64>,
    /// If true, remove dependencies; if false (default), add
    #[serde(default)]
    remove: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TaskIdsInput {
    task_ids: Vec<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LogArtifactInput {
    name: String,
    file_path: String,
    /// Task ID (defaults to active task)
    #[serde(default)]
    task_id: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GetArtifactsInput {
    #[serde(default)]
    task_id: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReorderTaskInput {
    id: i64,
    #[serde(default)]
    after_id: Option<i64>,
    #[serde(default)]
    before_id: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EmptyInput {}

#[derive(Debug, Deserialize, JsonSchema)]
struct AdvanceInput {
    #[serde(default)]
    dry_run: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ArchiveInput {}

#[derive(Debug, Deserialize, JsonSchema)]
struct SubtaskDefinition {
    title: String,
    description: String,
    dod: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SplitTaskInput {
    id: i64,
    subtasks: Vec<SubtaskDefinition>,
}

// ============================================================================
// Response Types
// ============================================================================

/// A lightweight summary of a task for list views.
/// Excludes large fields (description, dod) to keep responses concise.
/// Use get_task for full task details.
#[derive(Debug, Clone, serde::Serialize)]
struct TaskSummary {
    pub id: i64,
    pub title: String,
    pub status: crate::core::TaskStatus,
    pub manual_order: f64,
    pub created_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_touched_at: String,
}

impl From<crate::core::Task> for TaskSummary {
    fn from(task: crate::core::Task) -> Self {
        TaskSummary {
            id: task.id,
            title: task.title,
            status: task.status,
            manual_order: task.manual_order,
            created_at: task.created_at,
            started_at: task.started_at,
            completed_at: task.completed_at,
            last_touched_at: task.last_touched_at,
        }
    }
}

// ============================================================================
// Tool Handlers
// ============================================================================

/// Get current task handler
pub struct GetCurrentTaskHandler;

impl ToolHandler for GetCurrentTaskHandler {
    fn handle(&self, db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        match get_current_task(db) {
            Ok(task) => {
                // Also get artifacts for the current task
                let artifacts = get_task_artifacts(db, Some(task.id)).unwrap_or_default();
                Ok(McpResponse::ok(serde_json::json!({
                    "task": task,
                    "artifacts": artifacts
                })))
            }
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "get_current_task",
            "Returns the currently active task and its artifacts. Use this to check what you're currently working on.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Get target handler
pub struct GetTargetHandler;

impl ToolHandler for GetTargetHandler {
    fn handle(&self, db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        match get_target(db) {
            Ok(Some(task)) => Ok(McpResponse::ok(task)),
            Ok(None) => Ok(McpResponse::ok(serde_json::json!({
                "message": "No target set"
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "tt_get_target",
            "Returns the current target task. Returns a message if no target is set. This is the goal you're working toward.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Clear focus handler
pub struct ClearFocusHandler;

impl ToolHandler for ClearFocusHandler {
    fn handle(&self, db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        match clear_target(db) {
            Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                "message": "Focus cleared"
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "clear_focus",
            "Clear the current focus. After clearing, list_tasks will operate on all tasks.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Get task by ID handler
pub struct GetTaskHandler;

impl ToolHandler for GetTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: GetTaskInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match get_task_detail_allow_archived(db, input.id) {
            Ok(detail) => Ok(McpResponse::ok(detail)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(GetTaskInput);
        ToolMetadata::new(
            "get_task",
            "Get full details of a task. Use id=N to get a specific task, or current=true to get the active task.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Get multiple tasks by ID handler
pub struct GetTasksHandler;

impl ToolHandler for GetTasksHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: TaskIdsInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match get_tasks_allow_archived(db, input.task_ids) {
            Ok(tasks) => Ok(McpResponse::ok(tasks)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(TaskIdsInput);
        ToolMetadata::new(
            "get_tasks",
            "Get multiple tasks by their IDs. Returns only tasks that exist; non-existent IDs are silently ignored. Can view archived tasks.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// List tasks handler
pub struct ListTasksHandler;

impl ToolHandler for ListTasksHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: ListTasksInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        // Validate mutual exclusivity of status and active
        if input.status.is_some() && input.active {
            return Ok(McpResponse::error(
                "InvalidArgument",
                "Cannot use both status and active parameters together. Use active=true to show pending and in_progress tasks.",
            ));
        }

        // Parse status filter if provided
        let status_filter = match input.status {
            Some(s) => match s.to_lowercase().as_str() {
                "pending" => Some(crate::core::TaskStatus::Pending),
                "in_progress" => Some(crate::core::TaskStatus::InProgress),
                "completed" => Some(crate::core::TaskStatus::Completed),
                "blocked" => Some(crate::core::TaskStatus::Blocked),
                _ => {
                    return Ok(McpResponse::error(
                        "InvalidStatus",
                        &format!("Invalid status: {}", s),
                    ))
                }
            },
            None => None,
        };

        match list_tasks(
            db,
            input.no_focus,
            status_filter,
            input.active,
            input.limit,
            input.offset,
            input.archived,
        ) {
            Ok(tasks) => {
                // Filter by ids if provided
                let tasks = if let Some(ids) = input.ids {
                    tasks.into_iter().filter(|t| ids.contains(&t.id)).collect()
                } else {
                    tasks
                };
                let summaries: Vec<TaskSummary> =
                    tasks.into_iter().map(TaskSummary::from).collect();
                Ok(McpResponse::ok(summaries))
            }
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(ListTasksInput);
        ToolMetadata::new(
            "list_tasks",
            "List all tasks in sorted order. By default, shows only tasks in the target subgraph. Use all=true to see all tasks. Use archived=true to see only archived tasks. Use active=true to filter to pending and in_progress tasks. Can filter by status and paginate with limit/offset.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Create task handler
pub struct CreateTaskHandler;

impl ToolHandler for CreateTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: CreateTaskInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        let title = input.title.ok_or("title required")?;
        let description = input.description.ok_or("description required")?;
        let dod = input.dod.ok_or("dod required")?;

        let deps_opt = if input.depends_on.is_empty() {
            None
        } else {
            Some(input.depends_on)
        };

        match create_task(
            db,
            &title,
            &description,
            &dod,
            input.after_id,
            input.before_id,
            deps_opt,
        ) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(CreateTaskInput);
        ToolMetadata::new(
            "create_task",
            "Creates a new task. Use title/description/dod to define the task, and optionally specify after_id/before_id for positioning and depends_on for dependencies.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Edit task handler
pub struct EditTaskHandler;

impl ToolHandler for EditTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: EditTaskInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        // Handle dependencies
        if let Some(deps) = input.depends_on {
            if !deps.is_empty() {
                if let Err(e) = add_dependencies(db, input.id, deps) {
                    return Ok(McpResponse::from_tt_error(e));
                }
            }
        }

        if let Some(deps) = input.remove_depends_on {
            if !deps.is_empty() {
                for dep in deps {
                    let _ = remove_dependency(db, input.id, dep);
                }
            }
        }

        // Handle reordering
        if let Some(after_id) = input.after {
            if let Err(e) = reorder_task(db, input.id, Some(after_id), None) {
                return Ok(McpResponse::from_tt_error(e));
            }
        } else if let Some(before_id) = input.before {
            if let Err(e) = reorder_task(db, input.id, None, Some(before_id)) {
                return Ok(McpResponse::from_tt_error(e));
            }
        }

        // Parse status if provided
        let status_opt = match input.status {
            Some(s) => match s.to_lowercase().as_str() {
                "pending" => Some(crate::core::models::TaskStatus::Pending),
                "in_progress" => Some(crate::core::models::TaskStatus::InProgress),
                "completed" => Some(crate::core::models::TaskStatus::Completed),
                "blocked" => Some(crate::core::models::TaskStatus::Blocked),
                "cancelled" => Some(crate::core::models::TaskStatus::Cancelled),
                "split" => Some(crate::core::models::TaskStatus::Split),
                _ => {
                    return Ok(McpResponse::error(
                        "InvalidStatus",
                        &format!("Invalid status: {}", s),
                    ))
                }
            },
            None => None,
        };

        // Parse action if provided
        let action_opt = match input.action {
            Some(a) => match crate::core::models::EditAction::from_str(&a.to_lowercase()) {
                Ok(act) => Some(act),
                Err(_) => {
                    return Ok(McpResponse::error(
                        "InvalidAction",
                        &format!("Invalid action: {}", a),
                    ))
                }
            },
            None => None,
        };

        match edit_task(
            db,
            input.id,
            input.title.as_deref(),
            input.description.as_deref(),
            input.dod.as_deref(),
            status_opt,
            action_opt,
        ) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EditTaskInput);
        ToolMetadata::new(
            "edit_task",
            "Edit a task: change title/description/dod, set status, perform actions (complete/stop/cancel/block/unblock), add/remove dependencies (depends_on, remove_depends_on), or reorder (after, before).",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Set focus handler
pub struct SetFocusHandler;

impl ToolHandler for SetFocusHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match set_target(db, input.id) {
            Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                "message": "Focus set",
                "target_id": input.id
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(TaskIdInput);
        ToolMetadata::new(
            "set_focus",
            "Set the focus task you're working toward. This affects which tasks are shown in `list_tasks`.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Consolidated focus tool input
#[derive(Debug, Deserialize, JsonSchema)]
struct FocusInput {
    /// Action: set, get, clear
    action: String,
    /// Task ID (required for set action)
    #[serde(default)]
    id: Option<i64>,
}

/// Consolidated focus handler
pub struct FocusHandler;

impl ToolHandler for FocusHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: FocusInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match input.action.to_lowercase().as_str() {
            "set" => {
                let id = input.id.ok_or("id required for set action")?;
                match set_target(db, id) {
                    Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                        "message": "Focus set",
                        "target_id": id
                    }))),
                    Err(e) => Ok(McpResponse::from_tt_error(e)),
                }
            }
            "get" => match get_target(db) {
                Ok(Some(task)) => Ok(McpResponse::ok(task)),
                Ok(None) => Ok(McpResponse::ok(serde_json::json!({
                    "message": "No focus set"
                }))),
                Err(e) => Ok(McpResponse::from_tt_error(e)),
            },
            "clear" => match clear_target(db) {
                Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                    "message": "Focus cleared"
                }))),
                Err(e) => Ok(McpResponse::from_tt_error(e)),
            },
            "next" => {
                let all_tasks = crate::db::tasks::get_all_tasks(db).map_err(|e| e.to_string())?;
                let next_task = all_tasks
                    .iter()
                    .filter(|t| t.status == crate::core::TaskStatus::Pending)
                    .min_by(|a, b| {
                        a.manual_order
                            .partial_cmp(&b.manual_order)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });

                match next_task {
                    Some(task) => match set_target(db, task.id) {
                        Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                            "message": "Focus set",
                            "target_id": task.id,
                            "title": task.title
                        }))),
                        Err(e) => Ok(McpResponse::from_tt_error(e)),
                    },
                    None => Ok(McpResponse::ok(serde_json::json!({
                        "message": "No pending tasks available"
                    }))),
                }
            }
            "last" => {
                let all_tasks = crate::db::tasks::get_all_tasks(db).map_err(|e| e.to_string())?;
                match all_tasks.iter().max_by_key(|t| t.id) {
                    Some(task) => match set_target(db, task.id) {
                        Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                            "message": "Focus set",
                            "target_id": task.id,
                            "title": task.title
                        }))),
                        Err(e) => Ok(McpResponse::from_tt_error(e)),
                    },
                    None => Ok(McpResponse::ok(serde_json::json!({
                        "message": "No tasks available"
                    }))),
                }
            }
            _ => Ok(McpResponse::error(
                "InvalidAction",
                &format!(
                    "Invalid action: {}. Use set, get, clear, next, or last",
                    input.action
                ),
            )),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(FocusInput);
        ToolMetadata::new(
            "focus",
            "Manage focus task: set id to focus on a task, get to see current focus, clear to remove focus, next to auto-select lowest pending task, last to select most recent task.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Advance task handler - complete current and start next
pub struct AdvanceTaskHandler;

impl ToolHandler for AdvanceTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: AdvanceInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match advance_task(db, input.dry_run) {
            Ok(result) => Ok(McpResponse::ok(serde_json::json!({
                "completed": result.completed,
                "started": result.started,
                "dry_run": input.dry_run
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(AdvanceInput);
        ToolMetadata::new(
            "advance_task",
            "Complete the current task and start the next runnable task in one operation. Set dry_run=true to preview without executing. This validates DoD is present before completing.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Archive all completed tasks
pub struct ArchiveTasksHandler;

impl ToolHandler for ArchiveTasksHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let _input: ArchiveInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match archive_completed(db) {
            Ok(count) => Ok(McpResponse::ok(serde_json::json!({
                "archived_count": count
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(ArchiveInput);
        ToolMetadata::new(
            "archive_tasks",
            "Archive all completed tasks. This hides them from the default task list. Returns the count of archived tasks.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Block task handler (supports single or bulk)
pub struct BlockTaskHandler;

impl ToolHandler for BlockTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        // Try to parse as bulk (TaskIdsInput) first
        if let Ok(input) = serde_json::from_value::<TaskIdsInput>(params.clone()) {
            if input.task_ids.len() > 1 {
                let results = block_tasks(db, input.task_ids);
                let succeeded = results.iter().filter(|r| r.is_ok()).count();
                let failed = results.len() - succeeded;
                return Ok(McpResponse::ok(serde_json::json!({
                    "succeeded": succeeded,
                    "failed": failed,
                    "results": results_to_json(results)
                })));
            } else if input.task_ids.len() == 1 {
                match block_task(db, input.task_ids[0]) {
                    Ok(task) => return Ok(McpResponse::ok(task)),
                    Err(e) => return Ok(McpResponse::from_tt_error(e)),
                }
            }
        }

        // Fall back to single task ID
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match block_task(db, input.id) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        // Use TaskIdsInput for schema to show bulk is supported
        let schema = schemars::schema_for!(TaskIdsInput);
        ToolMetadata::new(
            "block_task",
            "Block one or more tasks from being started. Use this when tasks cannot proceed due to external dependencies or issues. If a task is active, it will be stopped first then blocked. Pass either 'id' for single task or 'task_ids' for multiple.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Unblock task handler (supports single or bulk)
pub struct UnblockTaskHandler;

impl ToolHandler for UnblockTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        // Try to parse as bulk (TaskIdsInput) first
        if let Ok(input) = serde_json::from_value::<TaskIdsInput>(params.clone()) {
            if input.task_ids.len() > 1 {
                let results = unblock_tasks(db, input.task_ids);
                let succeeded = results.iter().filter(|r| r.is_ok()).count();
                let failed = results.len() - succeeded;
                return Ok(McpResponse::ok(serde_json::json!({
                    "succeeded": succeeded,
                    "failed": failed,
                    "results": results_to_json(results)
                })));
            } else if input.task_ids.len() == 1 {
                match unblock_task(db, input.task_ids[0]) {
                    Ok(task) => return Ok(McpResponse::ok(task)),
                    Err(e) => return Ok(McpResponse::from_tt_error(e)),
                }
            }
        }

        // Fall back to single task ID
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match unblock_task(db, input.id) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        // Use TaskIdsInput for schema to show bulk is supported
        let schema = schemars::schema_for!(TaskIdsInput);
        ToolMetadata::new(
            "unblock_task",
            "Unblock one or more previously blocked tasks, returning them to pending status. Tasks must be in blocked state. Pass either 'id' for single task or 'task_ids' for multiple.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Helper to convert Vec<Result<Task>> to JSON
fn results_to_json(
    results: Vec<crate::error::Result<crate::core::models::Task>>,
) -> Vec<serde_json::Value> {
    results
        .into_iter()
        .map(|r| match r {
            Ok(task) => serde_json::json!({"ok": task}),
            Err(e) => serde_json::json!({"error": e.to_string()}),
        })
        .collect()
}

/// Manage dependency handler (add or remove)
pub struct ManageDependencyHandler;

impl ToolHandler for ManageDependencyHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: DependencyInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        if input.remove {
            // Remove dependencies
            if input.depends_on.len() == 1 {
                // Single dependency
                match remove_dependency(db, input.task_id, input.depends_on[0]) {
                    Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                        "message": "Dependency removed",
                        "task_id": input.task_id,
                        "depends_on": input.depends_on[0]
                    }))),
                    Err(e) => Ok(McpResponse::from_tt_error(e)),
                }
            } else {
                // Bulk dependencies
                match remove_dependencies(db, input.task_id, input.depends_on.clone()) {
                    Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                        "message": "Dependencies removed",
                        "task_id": input.task_id,
                        "depends_on": input.depends_on
                    }))),
                    Err(e) => Ok(McpResponse::from_tt_error(e)),
                }
            }
        } else {
            // Add dependencies
            if input.depends_on.len() == 1 {
                // Single dependency
                match add_dependency(db, input.task_id, input.depends_on[0]) {
                    Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                        "message": "Dependency added",
                        "task_id": input.task_id,
                        "depends_on": input.depends_on[0]
                    }))),
                    Err(e) => Ok(McpResponse::from_tt_error(e)),
                }
            } else {
                // Bulk dependencies
                match add_dependencies(db, input.task_id, input.depends_on.clone()) {
                    Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                        "message": "Dependencies added",
                        "task_id": input.task_id,
                        "depends_on": input.depends_on
                    }))),
                    Err(e) => Ok(McpResponse::from_tt_error(e)),
                }
            }
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(DependencyInput);
        ToolMetadata::new(
            "manage_dependency",
            "Add or remove dependencies for a task. By default, adds dependencies (task_id depends on depends_on). Set remove=true to remove dependencies instead. This means task_id cannot be started until all depends_on tasks are completed. This will fail if it would create a cycle.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Log artifact handler
pub struct LogArtifactHandler;

impl ToolHandler for LogArtifactHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: LogArtifactInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match log_artifact(db, &input.name, &input.file_path, input.task_id) {
            Ok(artifact) => Ok(McpResponse::ok(artifact)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(LogArtifactInput);
        ToolMetadata::new(
            "log_artifact",
            "Records a file you have created as an artifact of the current task. Create the file first, then call this. Use descriptive names like 'research', 'plan', 'implementation-notes', 'test-report'.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Get artifacts handler
pub struct GetArtifactsHandler;

impl ToolHandler for GetArtifactsHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: GetArtifactsInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match get_task_artifacts(db, input.task_id) {
            Ok(artifacts) => Ok(McpResponse::ok(artifacts)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(GetArtifactsInput);
        ToolMetadata::new(
            "get_artifacts",
            "Get all artifacts for a task. If no task_id is provided, returns artifacts for the currently active task.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Consolidated artifacts tool input
#[derive(Debug, Deserialize, JsonSchema)]
struct ArtifactsInput {
    /// Action: log, list
    action: String,
    /// Task ID (for list action, defaults to active task)
    #[serde(default)]
    id: Option<i64>,
    /// Artifact name (for log action)
    #[serde(default)]
    name: Option<String>,
    /// File path (for log action)
    #[serde(default)]
    file_path: Option<String>,
}

/// Consolidated artifacts handler
pub struct ArtifactsHandler;

impl ToolHandler for ArtifactsHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: ArtifactsInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match input.action.to_lowercase().as_str() {
            "log" => {
                let name = input.name.ok_or("name required for log action")?;
                let file_path = input.file_path.ok_or("file_path required for log action")?;
                match log_artifact(db, &name, &file_path, input.id) {
                    Ok(artifact) => Ok(McpResponse::ok(artifact)),
                    Err(e) => Ok(McpResponse::from_tt_error(e)),
                }
            }
            "list" => match get_task_artifacts(db, input.id) {
                Ok(artifacts) => Ok(McpResponse::ok(artifacts)),
                Err(e) => Ok(McpResponse::from_tt_error(e)),
            },
            _ => Ok(McpResponse::error(
                "InvalidAction",
                &format!("Invalid action: {}. Use list or log", input.action),
            )),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(ArtifactsInput);
        ToolMetadata::new(
            "artifacts",
            "Manage task artifacts: log to record a file as artifact, list to get artifacts for a task.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Reorder task handler
pub struct ReorderTaskHandler;

impl ToolHandler for ReorderTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: ReorderTaskInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match reorder_task(db, input.id, input.after_id, input.before_id) {
            Ok(new_order) => Ok(McpResponse::ok(serde_json::json!({
                "task_id": input.id,
                "new_order": new_order
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(ReorderTaskInput);
        ToolMetadata::new(
            "reorder_task",
            "Change the manual order of a task. Use after_id to place it after a specific task, or before_id to place it before. This affects the default sort order.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Split task handler
pub struct SplitTaskHandler;

impl ToolHandler for SplitTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: SplitTaskInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        // Convert subtask definitions to tuples for the core function
        let subtasks: Vec<(String, String, String)> = input
            .subtasks
            .into_iter()
            .map(|s| (s.title, s.description, s.dod))
            .collect();

        match split_task(db, input.id, subtasks) {
            Ok(new_tasks) => {
                // Return the created subtasks
                // Note: parent task is soft-deleted by split_task, so we don't return it
                Ok(McpResponse::ok(serde_json::json!({
                    "parent_id": input.id,
                    "subtasks": new_tasks
                })))
            }
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(SplitTaskInput);
        ToolMetadata::new(
            "split_task",
            "Split a task into multiple subtasks. The original task is marked as completed and new subtasks are created with the same dependencies.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

// ============================================================================
// Tool Registration
// ============================================================================

/// Register all tool handlers
pub fn register_all_tools() -> HandlerRegistry {
    let mut registry = HandlerRegistry::new();

    // Query tools
    registry.insert("get_task".to_string(), Box::new(GetTaskHandler));
    registry.insert("list_tasks".to_string(), Box::new(ListTasksHandler));

    // Task management tools
    registry.insert("create_task".to_string(), Box::new(CreateTaskHandler));
    registry.insert("edit_task".to_string(), Box::new(EditTaskHandler));
    registry.insert("focus".to_string(), Box::new(FocusHandler));
    registry.insert("archive_tasks".to_string(), Box::new(ArchiveTasksHandler));
    registry.insert("split_task".to_string(), Box::new(SplitTaskHandler));

    // Workflow tools
    registry.insert("advance_task".to_string(), Box::new(AdvanceTaskHandler));

    // Artifact tools
    registry.insert("artifacts".to_string(), Box::new(ArtifactsHandler));

    registry
}

/// Get tool metadata for all registered tools
pub fn get_all_tool_metadata(registry: &HandlerRegistry) -> Vec<ToolMetadata> {
    registry.values().map(|h| h.metadata()).collect()
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
    fn test_register_all_tools() {
        let registry = register_all_tools();
        assert_eq!(registry.len(), 9); // 9 tools

        // Check some key tools exist
        assert!(registry.contains_key("get_task"));
        assert!(registry.contains_key("create_task"));
    }

    #[test]
    fn test_get_tool_metadata() {
        let registry = register_all_tools();
        let metadata = get_all_tool_metadata(&registry);

        assert_eq!(metadata.len(), 9); // 9 tools

        // Check metadata structure
        for meta in metadata {
            assert!(!meta.name.is_empty());
            assert!(!meta.description.is_empty());
            assert!(meta.input_schema.is_object());
        }
    }

    #[test]
    fn test_create_task_handler() {
        let conn = setup();
        let handler = CreateTaskHandler;

        let params = serde_json::json!({
            "title": "Test Task",
            "description": "A test task",
            "dod": "Test DoD"
        });

        let result = handler.handle(&conn, params).unwrap();
        match result {
            McpResponse::Ok { data } => {
                assert_eq!(data["title"], "Test Task");
                assert_eq!(data["description"], "A test task");
            }
            _ => panic!("Expected Ok response"),
        }
    }

    #[test]
    fn test_get_task_handler() {
        let conn = setup();
        let create_handler = CreateTaskHandler;

        // Create a task first
        let params = serde_json::json!({
            "title": "Test Task",
            "description": "A test task",
            "dod": "Test DoD"
        });
        let result = create_handler.handle(&conn, params).unwrap();
        let task_id = match result {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok response"),
        };

        // Now get it
        let get_handler = GetTaskHandler;
        let params = serde_json::json!({"id": task_id});
        let result = get_handler.handle(&conn, params).unwrap();

        match result {
            McpResponse::Ok { data } => {
                assert!(data["task"].is_object());
            }
            _ => panic!("Expected Ok response"),
        }
    }

    #[test]
    fn test_get_task_not_found() {
        let conn = setup();
        let handler = GetTaskHandler;

        let params = serde_json::json!({"id": 999});
        let result = handler.handle(&conn, params).unwrap();

        match result {
            McpResponse::Error { error_code, .. } => {
                assert_eq!(error_code, "TaskNotFound");
            }
            _ => panic!("Expected Error response"),
        }
    }

    #[test]
    fn test_edit_task_status_workflow() {
        let conn = setup();

        // Create task
        let create = CreateTaskHandler;
        let result = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Test",
                    "description": "Test description",
                    "dod": "DoD"
                }),
            )
            .unwrap();
        let task_id = match result {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Set focus
        let set_focus = SetFocusHandler;
        set_focus
            .handle(&conn, serde_json::json!({"id": task_id}))
            .unwrap();

        // Start task using edit_task with status
        let edit = EditTaskHandler;
        let result = edit
            .handle(
                &conn,
                serde_json::json!({"id": task_id, "status": "in_progress"}),
            )
            .unwrap();
        match result {
            McpResponse::Ok { data } => {
                assert_eq!(data["status"], "in_progress");
            }
            _ => panic!("Expected Ok"),
        }

        // Complete task using edit_task with status
        let result = edit
            .handle(
                &conn,
                serde_json::json!({"id": task_id, "status": "completed"}),
            )
            .unwrap();
        match result {
            McpResponse::Ok { data } => {
                assert_eq!(data["status"], "completed");
            }
            _ => panic!("Expected Ok"),
        }
    }

    #[test]
    fn test_add_dependency() {
        let conn = setup();

        // Create two tasks
        let create = CreateTaskHandler;
        let task1 = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Task 1",
                    "description": "Task 1 description",
                    "dod": "Task 1 DoD"
                }),
            )
            .unwrap();
        let id1 = match task1 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let task2 = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Task 2",
                    "description": "Task 2 description",
                    "dod": "Task 2 DoD"
                }),
            )
            .unwrap();
        let id2 = match task2 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Add dependency
        let add_dep = ManageDependencyHandler;
        let result = add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id2,
                    "depends_on": [id1]
                }),
            )
            .unwrap();

        match result {
            McpResponse::Ok { data } => {
                assert_eq!(data["message"], "Dependency added");
            }
            _ => panic!("Expected Ok"),
        }
    }

    #[test]
    fn test_cycle_detection() {
        let conn = setup();

        // Create three tasks
        let create = CreateTaskHandler;
        let task1 = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Task 1",
                    "description": "Task 1 description",
                    "dod": "Task 1 DoD"
                }),
            )
            .unwrap();
        let id1 = match task1 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let task2 = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Task 2",
                    "description": "Task 2 description",
                    "dod": "Task 2 DoD"
                }),
            )
            .unwrap();
        let id2 = match task2 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let task3 = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Task 3",
                    "description": "Task 3 description",
                    "dod": "Task 3 DoD"
                }),
            )
            .unwrap();
        let id3 = match task3 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Add dependency chain: 1 -> 2 -> 3
        let add_dep = ManageDependencyHandler;
        add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id2,
                    "depends_on": [id1]
                }),
            )
            .unwrap();
        add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id3,
                    "depends_on": [id2]
                }),
            )
            .unwrap();

        // Try to add cycle: 3 -> 1
        let result = add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id1,
                    "depends_on": [id3]
                }),
            )
            .unwrap();

        match result {
            McpResponse::Error { error_code, .. } => {
                assert_eq!(error_code, "CycleDetected");
            }
            _ => panic!("Expected Error for cycle detection"),
        }
    }

    #[test]
    fn test_list_tasks() {
        let conn = setup();

        // Create a task and set target
        let create = CreateTaskHandler;
        let task = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Task 1",
                    "description": "Task 1 description",
                    "dod": "Task 1 DoD"
                }),
            )
            .unwrap();
        let id = match task {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let set_focus = SetFocusHandler;
        set_focus
            .handle(&conn, serde_json::json!({"id": id}))
            .unwrap();

        // List tasks
        let list = ListTasksHandler;
        let result = list
            .handle(&conn, serde_json::json!({"all": false}))
            .unwrap();

        match result {
            McpResponse::Ok { data } => {
                assert!(data.is_array());
                // Verify TaskSummary format - no description or dod fields
                let tasks = data.as_array().unwrap();
                assert_eq!(tasks.len(), 1);
                let task = &tasks[0];
                assert_eq!(task["title"], "Task 1");
                assert!(task["description"].is_null() || task.get("description").is_none());
                assert!(task["dod"].is_null() || task.get("dod").is_none());
                // Verify expected fields are present
                assert!(task["id"].is_i64());
                assert!(task["status"].is_string());
                assert!(task["created_at"].is_string());
            }
            _ => panic!("Expected Ok"),
        }
    }

    #[test]
    fn test_no_active_task() {
        let conn = setup();

        let get_current = GetCurrentTaskHandler;
        let result = get_current.handle(&conn, serde_json::Value::Null).unwrap();

        match result {
            McpResponse::Error { error_code, .. } => {
                assert_eq!(error_code, "NoActiveTask");
            }
            _ => panic!("Expected NoActiveTask error"),
        }
    }

    #[test]
    fn test_split_task_handler() {
        // TDD: Test for split_task functionality
        // This test will fail until SplitTaskHandler is implemented
        let conn = setup();

        // Create a task to split
        let create = CreateTaskHandler;
        let task = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Parent Task",
                    "description": "Task to be split",
                    "dod": "Original DoD"
                }),
            )
            .unwrap();
        let parent_id = match task {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Try to split the task - this should work once implemented
        let split_handler = SplitTaskHandler;
        let result = split_handler
            .handle(
                &conn,
                serde_json::json!({
                    "id": parent_id,
                    "subtasks": [
                        {
                            "title": "Subtask 1",
                            "description": "First subtask",
                            "dod": "Subtask 1 DoD"
                        },
                        {
                            "title": "Subtask 2",
                            "description": "Second subtask",
                            "dod": "Subtask 2 DoD"
                        }
                    ]
                }),
            )
            .unwrap();

        match result {
            McpResponse::Ok { data } => {
                // Should return the created subtasks
                assert!(data["subtasks"].is_array());
                assert_eq!(data["subtasks"].as_array().unwrap().len(), 2);
                // Parent task id should be returned
                assert_eq!(data["parent_id"], parent_id);
            }
            _ => panic!("Expected Ok response for split_task"),
        }
    }
}

#[cfg(test)]
mod persistence_tests {
    //! Tests for MCP persistence - using file database

    use super::*;
    use crate::db::schema::CREATE_SCHEMA_SQL;
    use rusqlite::Connection;
    use std::fs;
    use std::path::PathBuf;

    fn setup_file_db() -> PathBuf {
        let test_dir = "/tmp/tt_mcp_persist_test";
        let db_path = PathBuf::from(format!("{}/tt.db", test_dir));

        // Cleanup
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        // Initialize schema
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();
        }

        db_path
    }

    #[test]
    fn test_mcp_edit_task_status_persistence() {
        let db_path = setup_file_db();

        // Step 1: Create task (using our project's create_task via MCP handler)
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            let handler = CreateTaskHandler;
            let result = handler
                .handle(
                    &conn,
                    serde_json::json!({
                        "title": "Test Task",
                        "description": "Test description",
                        "dod": "Definition of done"
                    }),
                )
                .unwrap();

            let task_id = match result {
                McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
                _ => panic!("Expected Ok response"),
            };
            assert_eq!(task_id, 1);
        }

        // Verify creation with new connection
        {
            let conn = Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT status FROM tasks WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status, "pending");
        }

        // Step 2: Set target and start task using edit_task with status
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            // Set target
            crate::db::config::set_target(&conn, 1).unwrap();

            // Start task using edit_task with status
            let handler = EditTaskHandler;
            let result = handler
                .handle(&conn, serde_json::json!({"id": 1, "status": "in_progress"}))
                .unwrap();

            match result {
                McpResponse::Ok { data } => {
                    assert_eq!(data["status"], "in_progress");
                }
                _ => panic!("Expected Ok response"),
            }
        }

        // Step 3: Verify with NEW connection (this is where the bug should show)
        {
            let conn = Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT status FROM tasks WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();

            println!("Task status after connection close/reopen: {}", status);
            assert_eq!(
                status, "in_progress",
                "BUG: Task status should persist as 'in_progress' but was '{}'",
                status
            );
        }

        // Cleanup
        let _ = fs::remove_dir_all("/tmp/tt_mcp_persist_test");
    }

    #[test]
    fn test_split_task_persistence() {
        // TDD: Test that split_task creates subtasks that persist across connections

        let test_dir = "/tmp/tt_split_persist_test";
        let db_path = std::path::PathBuf::from(format!("{}/tt.db", test_dir));

        // Cleanup
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        // Initialize schema
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();
        }

        // Step 1: Create parent task
        let parent_id: i64;
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            let handler = CreateTaskHandler;
            let result = handler
                .handle(
                    &conn,
                    serde_json::json!({
                        "title": "Parent Task",
                        "description": "Task to split",
                        "dod": "Original DoD"
                    }),
                )
                .unwrap();

            parent_id = match result {
                McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
                _ => panic!("Expected Ok response"),
            };
        }

        // Step 2: Split the task - this will fail until SplitTaskHandler is implemented
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            let split_handler = SplitTaskHandler;
            let result = split_handler
                .handle(
                    &conn,
                    serde_json::json!({
                        "id": parent_id,
                        "subtasks": [
                            {
                                "title": "Subtask 1",
                                "description": "First subtask",
                                "dod": "Subtask 1 DoD"
                            },
                            {
                                "title": "Subtask 2",
                                "description": "Second subtask",
                                "dod": "Subtask 2 DoD"
                            }
                        ]
                    }),
                )
                .unwrap();

            match result {
                McpResponse::Ok { data } => {
                    assert!(data["subtasks"].is_array());
                    assert_eq!(data["subtasks"].as_array().unwrap().len(), 2);
                }
                _ => panic!("Expected Ok response for split_task persistence test"),
            }
        }

        // Step 3: Verify with NEW connection - subtasks should still exist
        {
            let conn = Connection::open(&db_path).unwrap();

            // Count tasks including deleted - should be 3 total (1 parent + 2 subtasks)
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 3, "Should have 3 tasks after splitting");

            // Verify parent has Split status
            let parent_status: String = conn
                .query_row(
                    "SELECT status FROM tasks WHERE id = ?",
                    [&parent_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                parent_status, "split",
                "Parent task should have Split status after split"
            );

            // Count non-deleted tasks - should be 3 (1 parent with Split status + 2 subtasks)
            let active_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM tasks WHERE deleted = 0", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(
                active_count, 3,
                "Should have 3 active tasks (parent with Split + 2 subtasks)"
            );
        }

        // Cleanup
        let _ = fs::remove_dir_all(test_dir);
    }
}

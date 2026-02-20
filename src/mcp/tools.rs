//! MCP tool handlers
//!
//! Implements all 16 MCP tools as defined in SPEC.md Section 13.2

use crate::core::{
    add_dependency, block_task, complete_task, create_task, edit_task, get_current_task,
    get_next_task, get_target, get_task_artifacts, get_task_detail, list_tasks, log_artifact,
    remove_dependency, reorder_task, set_target, start_task, stop_task, unblock_task,
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
    all: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateTaskInput {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    dod: Option<String>,
    #[serde(default)]
    after_id: Option<i64>,
    #[serde(default)]
    before_id: Option<i64>,
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
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TaskIdInput {
    id: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DependencyInput {
    task_id: i64,
    depends_on: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LogArtifactInput {
    name: String,
    file_path: String,
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

// ============================================================================
// Tool Handlers
// ============================================================================

/// Get next task handler
pub struct GetNextTaskHandler;

impl ToolHandler for GetNextTaskHandler {
    fn handle(&self, db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        match get_next_task(db) {
            Ok(Some(task)) => Ok(McpResponse::ok(task)),
            Ok(None) => Ok(McpResponse::ok(
                serde_json::json!({"message": "No next task available"}),
            )),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "get_next_task",
            "Returns the next task to work on toward the current target. Call this after completing a task. If the response is TargetReached, stop working and report to the user.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

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

/// Debug info handler - returns current working directory
pub struct DebugInfoHandler;

impl ToolHandler for DebugInfoHandler {
    fn handle(&self, _db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        Ok(McpResponse::ok(serde_json::json!({
            "current_working_dir": cwd,
            "debug_message": "This shows where the MCP server thinks it is running from"
        })))
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "debug_info",
            "Returns debug information about the MCP server environment.",
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
            Ok(None) => Ok(McpResponse::error(
                "NoTarget",
                "No target set. Use `set_target` first.",
            )),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "tt_get_target",
            "Returns the current target task. This is the goal you're working toward.",
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

        match get_task_detail(db, input.id) {
            Ok(detail) => Ok(McpResponse::ok(detail)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(GetTaskInput);
        ToolMetadata::new(
            "get_task",
            "Get full details of a specific task including its dependencies, dependents, and artifacts.",
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

        match list_tasks(db, input.all) {
            Ok(tasks) => Ok(McpResponse::ok(tasks)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(ListTasksInput);
        ToolMetadata::new(
            "list_tasks",
            "List all tasks in sorted order. By default, shows only tasks in the target subgraph. Use all=true to see all tasks.",
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

        match create_task(
            db,
            &input.title,
            input.description.as_deref(),
            input.dod.as_deref(),
            input.after_id,
            input.before_id,
        ) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(CreateTaskInput);
        ToolMetadata::new(
            "create_task",
            "Creates a new task. If you discover during implementation that a task needs to be broken into smaller pieces, create subtasks and add dependencies. You can specify after_id or before_id to position the task in the list.",
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

        match edit_task(
            db,
            input.id,
            input.title.as_deref(),
            input.description.as_deref(),
            input.dod.as_deref(),
        ) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EditTaskInput);
        ToolMetadata::new(
            "edit_task",
            "Edit an existing task's title, description, or definition of done. Only specified fields are updated.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Set target handler
pub struct SetTargetHandler;

impl ToolHandler for SetTargetHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match set_target(db, input.id) {
            Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                "message": "Target set",
                "target_id": input.id
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(TaskIdInput);
        ToolMetadata::new(
            "set_target",
            "Set the target task you're working toward. This affects which tasks are shown in `list_tasks` and `get_next_task`.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Start task handler
pub struct StartTaskHandler;

impl ToolHandler for StartTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match start_task(db, input.id) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(TaskIdInput);
        ToolMetadata::new(
            "start_task",
            "Start working on a task. The task must be pending, have no unmet dependencies, and there must be no other active task. If the task is already in progress, this is a no-op.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Stop task handler
pub struct StopTaskHandler;

impl ToolHandler for StopTaskHandler {
    fn handle(&self, db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        match stop_task(db) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "stop_task",
            "Stop the currently active task and return it to pending status. Call this if you need to switch to a different task.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Complete task handler
pub struct CompleteTaskHandler;

impl ToolHandler for CompleteTaskHandler {
    fn handle(&self, db: &Connection, _params: serde_json::Value) -> Result<McpResponse, String> {
        match complete_task(db) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => {
                eprintln!("DEBUG CompleteTaskHandler error: {:?}", e);
                Ok(McpResponse::from_tt_error(e))
            }
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(EmptyInput);
        ToolMetadata::new(
            "complete_task",
            "Complete the currently active task. The task must have a definition of done set. After completing, call `get_next_task` to get the next task to work on.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Block task handler
pub struct BlockTaskHandler;

impl ToolHandler for BlockTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match block_task(db, input.id) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(TaskIdInput);
        ToolMetadata::new(
            "block_task",
            "Block a task from being started. Use this when a task cannot proceed due to external dependencies or issues. If the task is active, it will be moved to blocked.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Unblock task handler
pub struct UnblockTaskHandler;

impl ToolHandler for UnblockTaskHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: TaskIdInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match unblock_task(db, input.id) {
            Ok(task) => Ok(McpResponse::ok(task)),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(TaskIdInput);
        ToolMetadata::new(
            "unblock_task",
            "Unblock a previously blocked task, returning it to pending status. The task must be in blocked state.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Add dependency handler
pub struct AddDependencyHandler;

impl ToolHandler for AddDependencyHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: DependencyInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match add_dependency(db, input.task_id, input.depends_on) {
            Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                "message": "Dependency added",
                "task_id": input.task_id,
                "depends_on": input.depends_on
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(DependencyInput);
        ToolMetadata::new(
            "add_dependency",
            "Add a dependency: task_id depends on depends_on. This means task_id cannot be started until depends_on is completed. This will fail if it would create a cycle.",
            serde_json::to_value(schema).unwrap(),
        )
    }
}

/// Remove dependency handler
pub struct RemoveDependencyHandler;

impl ToolHandler for RemoveDependencyHandler {
    fn handle(&self, db: &Connection, params: serde_json::Value) -> Result<McpResponse, String> {
        let input: DependencyInput =
            serde_json::from_value(params).map_err(|e| format!("Invalid parameters: {e}"))?;

        match remove_dependency(db, input.task_id, input.depends_on) {
            Ok(()) => Ok(McpResponse::ok(serde_json::json!({
                "message": "Dependency removed",
                "task_id": input.task_id,
                "depends_on": input.depends_on
            }))),
            Err(e) => Ok(McpResponse::from_tt_error(e)),
        }
    }

    fn metadata(&self) -> ToolMetadata {
        let schema = schemars::schema_for!(DependencyInput);
        ToolMetadata::new(
            "remove_dependency",
            "Remove a dependency relationship between two tasks. After this, task_id can be started even if depends_on is not completed.",
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

        match log_artifact(db, &input.name, &input.file_path) {
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

// ============================================================================
// Tool Registration
// ============================================================================

/// Register all tool handlers
pub fn register_all_tools() -> HandlerRegistry {
    let mut registry = HandlerRegistry::new();

    // Query tools
    registry.insert("get_next_task".to_string(), Box::new(GetNextTaskHandler));
    registry.insert(
        "get_current_task".to_string(),
        Box::new(GetCurrentTaskHandler),
    );
    registry.insert("get_task".to_string(), Box::new(GetTaskHandler));
    registry.insert("list_tasks".to_string(), Box::new(ListTasksHandler));
    registry.insert("debug_info".to_string(), Box::new(DebugInfoHandler));

    // Task management tools
    registry.insert("create_task".to_string(), Box::new(CreateTaskHandler));
    registry.insert("edit_task".to_string(), Box::new(EditTaskHandler));
    registry.insert("set_target".to_string(), Box::new(SetTargetHandler));

    // Workflow tools
    registry.insert("start_task".to_string(), Box::new(StartTaskHandler));
    registry.insert("stop_task".to_string(), Box::new(StopTaskHandler));
    registry.insert("complete_task".to_string(), Box::new(CompleteTaskHandler));
    registry.insert("block_task".to_string(), Box::new(BlockTaskHandler));
    registry.insert("unblock_task".to_string(), Box::new(UnblockTaskHandler));

    // Dependency tools
    registry.insert("add_dependency".to_string(), Box::new(AddDependencyHandler));
    registry.insert(
        "remove_dependency".to_string(),
        Box::new(RemoveDependencyHandler),
    );

    // Artifact tools
    registry.insert("log_artifact".to_string(), Box::new(LogArtifactHandler));
    registry.insert("get_artifacts".to_string(), Box::new(GetArtifactsHandler));

    // Ordering tools
    registry.insert("reorder_task".to_string(), Box::new(ReorderTaskHandler));

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
        assert_eq!(registry.len(), 18); // All 18 tools registered

        // Check some key tools exist
        assert!(registry.contains_key("get_next_task"));
        assert!(registry.contains_key("create_task"));
        assert!(registry.contains_key("start_task"));
        assert!(registry.contains_key("complete_task"));
    }

    #[test]
    fn test_get_tool_metadata() {
        let registry = register_all_tools();
        let metadata = get_all_tool_metadata(&registry);

        assert_eq!(metadata.len(), 18);

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
            "description": "A test task"
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
        let params = serde_json::json!({"title": "Test Task"});
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
    fn test_start_task_workflow() {
        let conn = setup();

        // Create task
        let create = CreateTaskHandler;
        let result = create
            .handle(
                &conn,
                serde_json::json!({
                    "title": "Test",
                    "dod": "DoD"
                }),
            )
            .unwrap();
        let task_id = match result {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Set target
        let set_target = SetTargetHandler;
        set_target
            .handle(&conn, serde_json::json!({"id": task_id}))
            .unwrap();

        // Start task
        let start = StartTaskHandler;
        let result = start
            .handle(&conn, serde_json::json!({"id": task_id}))
            .unwrap();
        match result {
            McpResponse::Ok { data } => {
                assert_eq!(data["status"], "in_progress");
            }
            _ => panic!("Expected Ok"),
        }

        // Complete task
        let complete = CompleteTaskHandler;
        let result = complete.handle(&conn, serde_json::Value::Null).unwrap();
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
            .handle(&conn, serde_json::json!({"title": "Task 1"}))
            .unwrap();
        let id1 = match task1 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let task2 = create
            .handle(&conn, serde_json::json!({"title": "Task 2"}))
            .unwrap();
        let id2 = match task2 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Add dependency
        let add_dep = AddDependencyHandler;
        let result = add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id2,
                    "depends_on": id1
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
            .handle(&conn, serde_json::json!({"title": "Task 1"}))
            .unwrap();
        let id1 = match task1 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let task2 = create
            .handle(&conn, serde_json::json!({"title": "Task 2"}))
            .unwrap();
        let id2 = match task2 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let task3 = create
            .handle(&conn, serde_json::json!({"title": "Task 3"}))
            .unwrap();
        let id3 = match task3 {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        // Add dependency chain: 1 -> 2 -> 3
        let add_dep = AddDependencyHandler;
        add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id2,
                    "depends_on": id1
                }),
            )
            .unwrap();
        add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id3,
                    "depends_on": id2
                }),
            )
            .unwrap();

        // Try to add cycle: 3 -> 1
        let result = add_dep
            .handle(
                &conn,
                serde_json::json!({
                    "task_id": id1,
                    "depends_on": id3
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
            .handle(&conn, serde_json::json!({"title": "Task 1"}))
            .unwrap();
        let id = match task {
            McpResponse::Ok { data } => data["id"].as_i64().unwrap(),
            _ => panic!("Expected Ok"),
        };

        let set_target = SetTargetHandler;
        set_target
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
    fn test_no_target() {
        let conn = setup();

        let get_next = GetNextTaskHandler;
        let result = get_next.handle(&conn, serde_json::Value::Null).unwrap();

        match result {
            McpResponse::Error { error_code, .. } => {
                assert_eq!(error_code, "NoTarget");
            }
            _ => panic!("Expected NoTarget error"),
        }
    }
}

#[cfg(test)]
mod persistence_tests {
    //! Tests for MCP persistence bug - using file database

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
    fn test_mcp_start_task_persistence() {
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

        // Step 2: Set target and start task (simulating MCP workflow)
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            // Set target
            crate::db::config::set_target(&conn, 1).unwrap();

            // Start task
            let handler = StartTaskHandler;
            let result = handler.handle(&conn, serde_json::json!({"id": 1})).unwrap();

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
    fn test_mcp_start_task_persistence_with_explicit_drop() {
        //! This test explicitly drops the connection and verifies persistence
        let test_dir = "/tmp/tt_mcp_persist_test2";
        let db_path = PathBuf::from(format!("{}/tt.db", test_dir));

        // Cleanup
        let _ = fs::remove_dir_all(test_dir);
        fs::create_dir_all(test_dir).unwrap();

        // Initialize schema
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(CREATE_SCHEMA_SQL).unwrap();
        }

        // Step 1: Create task
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            let handler = CreateTaskHandler;
            let result = handler
                .handle(
                    &conn,
                    serde_json::json!({
                        "title": "Test Task",
                        "dod": "Definition of done"
                    }),
                )
                .unwrap();

            match result {
                McpResponse::Ok { data } => {
                    assert_eq!(data["status"], "pending");
                }
                _ => panic!("Expected Ok response"),
            }

            // conn goes out of scope and is dropped here
        }

        // Step 2: Verify creation with NEW connection
        {
            let conn = Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT status FROM tasks WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status, "pending", "Task should be pending after creation");
        }

        // Step 3: Start task (simulating MCP workflow with explicit drop)
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            // Set target first
            crate::db::config::set_target(&conn, 1).unwrap();

            // Start task using handler
            let handler = StartTaskHandler;
            let result = handler.handle(&conn, serde_json::json!({"id": 1})).unwrap();

            match result {
                McpResponse::Ok { data } => {
                    assert_eq!(
                        data["status"], "in_progress",
                        "Handler should return in_progress"
                    );
                }
                _ => panic!("Expected Ok response, got: {:?}", result),
            }

            // Verify within same connection
            let status: String = conn
                .query_row("SELECT status FROM tasks WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(
                status, "in_progress",
                "Task should be in_progress within connection"
            );

            // conn goes out of scope and is dropped here
        }

        // Step 4: Verify with NEW connection (this is where the bug would show)
        {
            let conn = Connection::open(&db_path).unwrap();
            let (status, started_at): (String, Option<String>) = conn
                .query_row(
                    "SELECT status, started_at FROM tasks WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();

            println!(
                "After connection drop/reopen - status: {}, started_at: {:?}",
                status, started_at
            );

            assert_eq!(
                status, "in_progress",
                "BUG REPRODUCED: Task status should be 'in_progress' but was '{}'",
                status
            );
            assert!(
                started_at.is_some(),
                "BUG REPRODUCED: started_at should be set but was None"
            );
        }

        // Step 5: Try stop_task
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

            let handler = StopTaskHandler;
            let result = handler.handle(&conn, serde_json::json!({})).unwrap();

            match result {
                McpResponse::Ok { data } => {
                    assert_eq!(data["status"], "pending", "Handler should return pending");
                }
                _ => panic!("Expected Ok response"),
            }
        }

        // Step 6: Verify stop persisted
        {
            let conn = Connection::open(&db_path).unwrap();
            let status: String = conn
                .query_row("SELECT status FROM tasks WHERE id = 1", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(status, "pending", "Task should be pending after stop");
        }

        // Cleanup
        let _ = fs::remove_dir_all(test_dir);
    }
}

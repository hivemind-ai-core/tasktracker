//! Core business logic for tt
//!
//! Pure business logic with no I/O dependencies. All functions are testable
//! without a database.

pub mod graph;
pub mod models;
pub mod operations;
pub mod ordering;
pub mod state;
pub mod target;

pub use graph::{
    find_cycle_path, find_order_conflicts, topological_sort, transitive_dependencies,
    would_create_cycle,
};
pub use models::{Artifact, Dependency, Task, TaskDetail, TaskStatus};
pub use operations::{
    add_dependencies, add_dependency, advance_task, archive_completed, block_task, block_tasks,
    cancel_task, clear_target, complete_task, create_task, edit_task, find_next_runnable,
    get_current_task, get_next_task, get_status, get_target, get_task_artifacts, get_task_detail,
    get_task_detail_allow_archived, get_tasks, get_tasks_allow_archived, list_tasks, log_artifact,
    reindex_tasks, remove_dependencies, remove_dependency, reorder_task, set_target, split_task,
    start_task, stop_task, unblock_task, unblock_tasks, AdvanceResult, StatusOutput, StatusSummary,
};
pub use ordering::{calculate_manual_order, reindex_orders, ORDER_GAP};
pub use state::{
    can_block_task, can_cancel_task, can_complete_task, can_start_task, can_stop_task,
    can_unblock_task, validate_transition,
};
pub use target::{
    all_remaining_blocked, compute_target_subgraph, find_next_task, get_blocked_tasks,
    is_target_reached,
};

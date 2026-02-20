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
    add_dependency, block_task, complete_task, create_task, edit_task, get_current_task,
    get_next_task, get_target, get_task_artifacts, get_task_detail, list_tasks, log_artifact,
    reindex_tasks, remove_dependency, reorder_task, set_target, start_task, stop_task,
    unblock_task,
};
pub use ordering::{calculate_manual_order, reindex_orders, ORDER_GAP};
pub use state::{
    can_block_task, can_complete_task, can_start_task, can_stop_task, can_unblock_task,
    validate_transition,
};
pub use target::{
    all_remaining_blocked, compute_target_subgraph, find_next_task, get_blocked_tasks,
    is_target_reached,
};

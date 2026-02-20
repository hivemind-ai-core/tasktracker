//! Target subgraph computation
//!
//! Computes the set of tasks that need to be completed to reach a target.
//! Per SPEC.md Section 8.

use crate::core::models::{Dependency, Task, TaskStatus};
use std::collections::{HashMap, HashSet};

/// Compute the active subgraph for a target
///
/// Returns all tasks that are transitive dependencies of the target,
/// excluding completed tasks.
///
/// Algorithm:
/// 1. Start with the target task
/// 2. Recursively follow dependency edges (task depends_on -> prerequisite)
/// 3. Filter out completed tasks
/// 4. Return the set of active tasks
pub fn compute_target_subgraph(target_id: i64, tasks: &[Task], deps: &[Dependency]) -> Vec<Task> {
    // Build task lookup
    let task_map: HashMap<i64, &Task> = tasks.iter().map(|t| (t.id, t)).collect();

    // Build reverse graph: task -> what it depends on (prerequisites)
    let mut prereq_map: HashMap<i64, Vec<i64>> = HashMap::new();
    for dep in deps {
        prereq_map
            .entry(dep.task_id)
            .or_default()
            .push(dep.depends_on);
    }

    // BFS/DFS to find all transitive prerequisites
    let mut visited = HashSet::new();
    let mut stack = vec![target_id];
    let mut result = Vec::new();

    while let Some(current_id) = stack.pop() {
        if visited.insert(current_id) {
            if let Some(&task) = task_map.get(&current_id) {
                // Include the task if it's not completed
                if task.status != TaskStatus::Completed {
                    result.push(task.clone());
                }

                // Add prerequisites to search
                if let Some(prereqs) = prereq_map.get(&current_id) {
                    for &prereq_id in prereqs {
                        if !visited.contains(&prereq_id) {
                            stack.push(prereq_id);
                        }
                    }
                }
            }
        }
    }

    result
}

/// Check if the target has been reached (all tasks in subgraph are completed)
pub fn is_target_reached(target_id: i64, tasks: &[Task], deps: &[Dependency]) -> bool {
    let subgraph = compute_target_subgraph(target_id, tasks, deps);
    subgraph.is_empty()
}

/// Find the next ready task in the target subgraph
///
/// Returns the first task (by topological sort) that is:
/// - In the target subgraph
/// - Has status pending
/// - Has all dependencies completed
///
/// Returns None if no such task exists.
pub fn find_next_task(target_id: i64, tasks: &[Task], deps: &[Dependency]) -> Option<Task> {
    use crate::core::graph::topological_sort;

    // Get the subgraph (non-completed tasks)
    let subgraph_tasks = compute_target_subgraph(target_id, tasks, deps);

    if subgraph_tasks.is_empty() {
        return None;
    }

    // Build dependency list for subgraph only
    let subgraph_ids: HashSet<i64> = subgraph_tasks.iter().map(|t| t.id).collect();
    let subgraph_deps: Vec<Dependency> = deps
        .iter()
        .filter(|d| subgraph_ids.contains(&d.task_id) && subgraph_ids.contains(&d.depends_on))
        .cloned()
        .collect();

    // Topologically sort the subgraph
    let sorted = topological_sort(subgraph_tasks, &subgraph_deps).ok()?;

    // Find first pending task with all deps completed
    for task in sorted {
        if task.status == TaskStatus::Pending {
            // Check if all dependencies are completed
            let task_deps: Vec<&Dependency> =
                deps.iter().filter(|d| d.task_id == task.id).collect();
            let all_deps_completed = task_deps.iter().all(|d| {
                tasks
                    .iter()
                    .find(|t| t.id == d.depends_on)
                    .map(|t| t.status == TaskStatus::Completed)
                    .unwrap_or(true) // If dep not found, consider it completed
            });

            if all_deps_completed {
                return Some(task);
            }
        }
    }

    None
}

/// Get all blocked tasks in the target subgraph
///
/// Returns tasks with status Blocked that are in the target subgraph.
pub fn get_blocked_tasks(target_id: i64, tasks: &[Task], deps: &[Dependency]) -> Vec<Task> {
    let subgraph = compute_target_subgraph(target_id, tasks, deps);
    subgraph
        .into_iter()
        .filter(|t| t.status == TaskStatus::Blocked)
        .collect()
}

/// Check if all remaining tasks in subgraph are blocked
pub fn all_remaining_blocked(target_id: i64, tasks: &[Task], deps: &[Dependency]) -> bool {
    let subgraph = compute_target_subgraph(target_id, tasks, deps);

    // If subgraph is empty, target is reached (not "all blocked")
    if subgraph.is_empty() {
        return false;
    }

    // Check if all non-completed tasks are blocked
    subgraph
        .iter()
        .all(|t| t.status == TaskStatus::Blocked || t.status == TaskStatus::Completed)
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
            manual_order: id as f64 * 10.0,
            created_at: "2025-06-01T10:00:00".to_string(),
            started_at: None,
            completed_at: None,
            last_touched_at: "2025-06-01T10:00:00".to_string(),
        }
    }

    #[test]
    fn test_subgraph_empty() {
        let tasks = vec![];
        let deps = vec![];
        let result = compute_target_subgraph(1, &tasks, &deps);
        assert!(result.is_empty());
    }

    #[test]
    fn test_subgraph_single_target_no_deps() {
        let tasks = vec![make_task(1, TaskStatus::Pending)];
        let deps = vec![];
        let result = compute_target_subgraph(1, &tasks, &deps);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 1);
    }

    #[test]
    fn test_subgraph_linear_chain() {
        // T -> A -> B -> C (C is target, depends on B, B on A, A on T)
        let tasks = vec![
            make_task(4, TaskStatus::Pending), // Target
            make_task(3, TaskStatus::Pending),
            make_task(2, TaskStatus::Pending),
            make_task(1, TaskStatus::Pending),
        ];
        let deps = vec![
            Dependency {
                task_id: 4,
                depends_on: 3,
            },
            Dependency {
                task_id: 3,
                depends_on: 2,
            },
            Dependency {
                task_id: 2,
                depends_on: 1,
            },
        ];

        let result = compute_target_subgraph(4, &tasks, &deps);
        assert_eq!(result.len(), 4);

        let ids: HashSet<i64> = result.iter().map(|t| t.id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
        assert!(ids.contains(&4));
    }

    #[test]
    fn test_subgraph_excludes_completed() {
        // T -> A (A is completed, should not appear in subgraph)
        let tasks = vec![
            make_task(2, TaskStatus::Pending),   // Target
            make_task(1, TaskStatus::Completed), // Completed prerequisite
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        let result = compute_target_subgraph(2, &tasks, &deps);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, 2);
    }

    #[test]
    fn test_is_target_reached() {
        let tasks = vec![
            make_task(2, TaskStatus::Completed),
            make_task(1, TaskStatus::Completed),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        assert!(is_target_reached(2, &tasks, &deps));
    }

    #[test]
    fn test_is_target_not_reached() {
        let tasks = vec![
            make_task(2, TaskStatus::Pending),
            make_task(1, TaskStatus::Completed),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        assert!(!is_target_reached(2, &tasks, &deps));
    }

    #[test]
    fn test_find_next_task_simple() {
        // T -> A, A is pending
        let tasks = vec![
            make_task(2, TaskStatus::Pending),
            make_task(1, TaskStatus::Pending),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        let next = find_next_task(2, &tasks, &deps);
        assert!(next.is_some());
        assert_eq!(next.unwrap().id, 1); // A is next
    }

    #[test]
    fn test_find_next_task_no_ready() {
        // T -> A, A is blocked
        let tasks = vec![
            make_task(2, TaskStatus::Pending),
            make_task(1, TaskStatus::Blocked),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        let next = find_next_task(2, &tasks, &deps);
        assert!(next.is_none());
    }

    #[test]
    fn test_find_next_task_target_reached() {
        let tasks = vec![
            make_task(2, TaskStatus::Completed),
            make_task(1, TaskStatus::Completed),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        let next = find_next_task(2, &tasks, &deps);
        assert!(next.is_none());
    }

    #[test]
    fn test_get_blocked_tasks() {
        let tasks = vec![
            make_task(3, TaskStatus::Blocked),
            make_task(2, TaskStatus::Pending),
            make_task(1, TaskStatus::Completed),
        ];
        let deps = vec![
            Dependency {
                task_id: 3,
                depends_on: 2,
            },
            Dependency {
                task_id: 2,
                depends_on: 1,
            },
        ];

        let blocked = get_blocked_tasks(3, &tasks, &deps);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].id, 3);
    }

    #[test]
    fn test_all_remaining_blocked() {
        let tasks = vec![
            make_task(2, TaskStatus::Blocked),
            make_task(1, TaskStatus::Blocked),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        assert!(all_remaining_blocked(2, &tasks, &deps));
    }

    #[test]
    fn test_not_all_blocked_pending_exists() {
        let tasks = vec![
            make_task(2, TaskStatus::Pending),
            make_task(1, TaskStatus::Pending),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        assert!(!all_remaining_blocked(2, &tasks, &deps));
    }

    #[test]
    fn test_not_all_blocked_target_reached() {
        let tasks = vec![
            make_task(2, TaskStatus::Completed),
            make_task(1, TaskStatus::Completed),
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        // When target is reached, it's not "all blocked"
        assert!(!all_remaining_blocked(2, &tasks, &deps));
    }
}

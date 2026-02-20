//! Graph operations for the task DAG
//!
//! Implements Kahn's algorithm for topological sort and DFS for cycle detection.

use crate::core::models::{Dependency, Task};
use crate::error::{Error, Result};
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Min-heap wrapper for tasks ordered by manual_order
#[derive(Debug, Clone)]
struct OrderedTask {
    task: Task,
}

impl OrderedTask {
    fn new(task: Task) -> Self {
        Self { task }
    }
}

impl PartialEq for OrderedTask {
    fn eq(&self, other: &Self) -> bool {
        self.task.manual_order == other.task.manual_order
    }
}

impl Eq for OrderedTask {}

// Reverse ordering for min-heap (BinaryHeap is max-heap by default)
impl Ord for OrderedTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .task
            .manual_order
            .partial_cmp(&self.task.manual_order)
            .unwrap()
    }
}

impl PartialOrd for OrderedTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Build adjacency list from tasks and dependencies
fn build_graph(tasks: &[Task], deps: &[Dependency]) -> HashMap<i64, Vec<i64>> {
    let mut graph: HashMap<i64, Vec<i64>> = HashMap::new();

    // Initialize with all tasks
    for task in tasks {
        graph.entry(task.id).or_default();
    }

    // Add edges: depends_on -> task_id (prereq points to dependent)
    for dep in deps {
        graph.entry(dep.depends_on).or_default().push(dep.task_id);
    }

    graph
}

/// Build in-degree map from dependencies
fn build_in_degrees(tasks: &[Task], deps: &[Dependency]) -> HashMap<i64, usize> {
    let mut in_degrees: HashMap<i64, usize> = HashMap::new();

    // Initialize all tasks with 0
    for task in tasks {
        in_degrees.insert(task.id, 0);
    }

    // Count dependencies for each task
    for dep in deps {
        *in_degrees.entry(dep.task_id).or_default() += 1;
    }

    in_degrees
}

/// Topological sort using Kahn's algorithm with min-heap for manual_order
///
/// Returns tasks sorted topologically, with tie-breaking by manual_order.
/// Per SPEC.md Section 9.1.
pub fn topological_sort(tasks: Vec<Task>, deps: &[Dependency]) -> Result<Vec<Task>> {
    if tasks.is_empty() {
        return Ok(vec![]);
    }

    let graph = build_graph(&tasks, deps);
    let mut in_degrees = build_in_degrees(&tasks, deps);

    // Create task lookup
    let task_map: HashMap<i64, Task> = tasks.into_iter().map(|t| (t.id, t)).collect();

    // Min-heap for ready tasks (in-degree 0), ordered by manual_order
    let mut heap: BinaryHeap<OrderedTask> = BinaryHeap::new();

    // Seed heap with tasks that have no dependencies
    for (task_id, degree) in &in_degrees {
        if *degree == 0 {
            if let Some(task) = task_map.get(task_id) {
                heap.push(OrderedTask::new(task.clone()));
            }
        }
    }

    let mut result = Vec::new();

    while let Some(ordered) = heap.pop() {
        let task = ordered.task;
        result.push(task.clone());

        // Process dependents
        if let Some(dependents) = graph.get(&task.id) {
            for dependent_id in dependents {
                let degree = in_degrees.get_mut(dependent_id).unwrap();
                *degree -= 1;

                if *degree == 0 {
                    if let Some(dep_task) = task_map.get(dependent_id) {
                        heap.push(OrderedTask::new(dep_task.clone()));
                    }
                }
            }
        }
    }

    // Check if all tasks were processed (cycle detection)
    if result.len() != task_map.len() {
        // Find tasks that weren't processed (part of a cycle)
        let processed: HashSet<i64> = result.iter().map(|t| t.id).collect();
        let cycle_tasks: Vec<i64> = task_map
            .keys()
            .filter(|id| !processed.contains(*id))
            .copied()
            .collect();

        return Err(Error::CycleDetected(
            cycle_tasks.first().copied().unwrap_or(0),
            cycle_tasks.get(1).copied().unwrap_or(0),
            cycle_tasks,
        ));
    }

    Ok(result)
}

/// Detect if adding an edge would create a cycle using DFS
///
/// Returns true if adding edge (from -> to) would create a cycle.
/// Per SPEC.md Section 10.
pub fn would_create_cycle(from: i64, to: i64, deps: &[Dependency]) -> bool {
    // If from == to, it's a self-loop
    if from == to {
        return true;
    }

    // Build prereq graph: task -> what it depends on (prerequisites)
    // If we add edge (from -> to), we'd be saying "from depends on to"
    // This creates a cycle if there's already a path from to -> from
    // Following: to depends_on X, X depends_on Y, ... eventually reaches from
    let mut prereq_graph: HashMap<i64, Vec<i64>> = HashMap::new();
    for dep in deps {
        prereq_graph
            .entry(dep.task_id)
            .or_default()
            .push(dep.depends_on);
    }

    // DFS from 'to' to see if we can reach 'from' by following dependency chain
    let mut visited = HashSet::new();
    let mut stack = vec![to];

    while let Some(current) = stack.pop() {
        if current == from {
            return true;
        }

        if visited.insert(current) {
            if let Some(prereqs) = prereq_graph.get(&current) {
                for &prereq in prereqs {
                    stack.push(prereq);
                }
            }
        }
    }

    false
}

/// Find the cycle path if one exists when adding edge (from -> to)
///
/// Returns the cycle as a vector of task IDs starting and ending with 'from',
/// or None if no cycle. Format: [from, to, ..., from]
pub fn find_cycle_path(from: i64, to: i64, deps: &[Dependency]) -> Option<Vec<i64>> {
    if from == to {
        return Some(vec![from, to]);
    }

    // Build prereq graph: task -> what it depends on
    // We want to find a path from 'to' to 'from' by following dependencies
    let mut prereq_graph: HashMap<i64, Vec<i64>> = HashMap::new();
    for dep in deps {
        prereq_graph
            .entry(dep.task_id)
            .or_default()
            .push(dep.depends_on);
    }

    // DFS with parent tracking to find path from 'to' to 'from'
    let mut visited = HashSet::new();
    let mut parent: HashMap<i64, i64> = HashMap::new();
    let mut stack = vec![to];

    while let Some(current) = stack.pop() {
        if current == from {
            // Reconstruct path from 'from' back to 'to'
            let mut path = vec![from];
            let mut node = from;
            while let Some(&p) = parent.get(&node) {
                path.push(p);
                node = p;
                if node == to {
                    break;
                }
            }
            // path is [from, ..., to] (forwards through parent chain)
            // The cycle: from -> to -> ... -> from
            path.push(from); // Close the cycle
            return Some(path);
        }

        if visited.insert(current) {
            if let Some(prereqs) = prereq_graph.get(&current) {
                for &prereq in prereqs {
                    if !visited.contains(&prereq) {
                        parent.insert(prereq, current);
                        stack.push(prereq);
                    }
                }
            }
        }
    }

    None
}

/// Get all transitive dependencies (ancestors) of a task
///
/// Returns all tasks that the given task depends on, directly or indirectly.
pub fn transitive_dependencies(task_id: i64, deps: &[Dependency]) -> Vec<i64> {
    let mut prereq_graph: HashMap<i64, Vec<i64>> = HashMap::new();
    for dep in deps {
        prereq_graph
            .entry(dep.task_id)
            .or_default()
            .push(dep.depends_on);
    }

    let mut visited = HashSet::new();
    let mut stack = vec![task_id];
    let mut result = Vec::new();

    while let Some(current) = stack.pop() {
        if visited.insert(current) && current != task_id {
            result.push(current);
        }

        if let Some(prereqs) = prereq_graph.get(&current) {
            for &prereq in prereqs {
                if !visited.contains(&prereq) {
                    stack.push(prereq);
                }
            }
        }
    }

    result
}

/// Check for order conflicts where a task has lower manual_order than its dependency
///
/// Returns a list of (task_id, task_order, dep_id, dep_order) for conflicts.
/// Per SPEC.md Section 7.3.
pub fn find_order_conflicts(tasks: &[Task], deps: &[Dependency]) -> Vec<(i64, f64, i64, f64)> {
    let task_map: HashMap<i64, &Task> = tasks.iter().map(|t| (t.id, t)).collect();
    let mut conflicts = Vec::new();

    for dep in deps {
        if let (Some(task), Some(prereq)) =
            (task_map.get(&dep.task_id), task_map.get(&dep.depends_on))
        {
            // Task should have higher manual_order than its prerequisite
            // If task.manual_order < prereq.manual_order, it's a conflict
            if task.manual_order < prereq.manual_order {
                conflicts.push((task.id, task.manual_order, prereq.id, prereq.manual_order));
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::TaskStatus;

    fn make_task(id: i64, order: f64) -> Task {
        Task {
            id,
            title: format!("Task {id}"),
            description: None,
            dod: None,
            status: TaskStatus::Pending,
            manual_order: order,
            created_at: "2025-06-01T10:00:00".to_string(),
            started_at: None,
            completed_at: None,
            last_touched_at: "2025-06-01T10:00:00".to_string(),
        }
    }

    #[test]
    fn test_topological_sort_empty() {
        let result = topological_sort(vec![], &[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_topological_sort_linear_chain() {
        // A -> B -> C (A must come before B before C)
        let tasks = vec![
            make_task(1, 10.0), // A
            make_task(2, 20.0), // B
            make_task(3, 30.0), // C
        ];
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            }, // B depends on A
            Dependency {
                task_id: 3,
                depends_on: 2,
            }, // C depends on B
        ];

        let result = topological_sort(tasks, &deps).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, 1);
        assert_eq!(result[1].id, 2);
        assert_eq!(result[2].id, 3);
    }

    #[test]
    fn test_topological_sort_diamond() {
        //     A
        //    / \
        //   B   C
        //    \ /
        //     D
        let tasks = vec![
            make_task(1, 10.0), // A
            make_task(2, 20.0), // B
            make_task(3, 30.0), // C
            make_task(4, 40.0), // D
        ];
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            }, // B depends on A
            Dependency {
                task_id: 3,
                depends_on: 1,
            }, // C depends on A
            Dependency {
                task_id: 4,
                depends_on: 2,
            }, // D depends on B
            Dependency {
                task_id: 4,
                depends_on: 3,
            }, // D depends on C
        ];

        let result = topological_sort(tasks, &deps).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].id, 1); // A first
        assert_eq!(result[3].id, 4); // D last
    }

    #[test]
    fn test_topological_sort_manual_order_tiebreak() {
        // Two independent tasks, manual_order decides
        let tasks = vec![
            make_task(1, 30.0), // Higher order
            make_task(2, 20.0), // Lower order (should come first)
        ];
        let deps = vec![];

        let result = topological_sort(tasks, &deps).unwrap();
        assert_eq!(result[0].id, 2); // Lower order first
        assert_eq!(result[1].id, 1); // Higher order second
    }

    #[test]
    fn test_cycle_detection_direct() {
        // A -> B, B -> A (cycle)
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            },
            Dependency {
                task_id: 1,
                depends_on: 2,
            },
        ];

        assert!(would_create_cycle(1, 2, &deps)); // Would create another edge in cycle
    }

    #[test]
    fn test_cycle_detection_indirect() {
        // A -> B -> C -> A (cycle)
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            },
            Dependency {
                task_id: 3,
                depends_on: 2,
            },
            Dependency {
                task_id: 1,
                depends_on: 3,
            },
        ];

        assert!(would_create_cycle(1, 2, &deps));
    }

    #[test]
    fn test_no_cycle() {
        // A -> B -> C (no cycle)
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            },
            Dependency {
                task_id: 3,
                depends_on: 2,
            },
        ];

        assert!(!would_create_cycle(4, 5, &deps)); // New edge between new nodes
    }

    #[test]
    fn test_would_create_cycle_new_edge() {
        // A -> B -> C (chain), adding C -> A would create cycle
        // Dependencies: 2 depends on 1, 3 depends on 2
        // So: 3 -> 2 -> 1 (C depends on B depends on A)
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            },
            Dependency {
                task_id: 3,
                depends_on: 2,
            },
        ];

        // Adding 3 -> 1 would mean 3 depends on 1
        // But 3 already transitively depends on 1 (via 2), so this is redundant
        // It does NOT create a cycle - there's no path from 1 to 3!
        assert!(!would_create_cycle(3, 1, &deps));

        // Adding 1 -> 3 would mean 1 depends on 3
        // Since 3 transitively depends on 1, this creates a cycle: 1 -> 3 -> 2 -> 1
        assert!(would_create_cycle(1, 3, &deps));
    }

    #[test]
    fn test_find_cycle_path() {
        // A -> B -> C -> A (cycle: 1 depends on 3, 2 depends on 1, 3 depends on 2)
        let deps = vec![
            Dependency {
                task_id: 1,
                depends_on: 3,
            }, // 1 depends on 3 (closes the cycle)
            Dependency {
                task_id: 2,
                depends_on: 1,
            }, // 2 depends on 1
            Dependency {
                task_id: 3,
                depends_on: 2,
            }, // 3 depends on 2
        ];

        // Check if adding 1 -> 3 would create cycle (it already exists, so checking the existing cycle)
        let path = find_cycle_path(1, 3, &deps).unwrap();
        assert!(path.contains(&1));
        assert!(path.contains(&2));
        assert!(path.contains(&3));
        assert_eq!(path[0], 1); // Starts with 'from'
        assert_eq!(path[path.len() - 1], 1); // Ends with 'from' (closed cycle)
    }

    #[test]
    fn test_transitive_dependencies() {
        // A -> B -> C -> D (D depends on C depends on B depends on A)
        let deps = vec![
            Dependency {
                task_id: 2,
                depends_on: 1,
            }, // B -> A
            Dependency {
                task_id: 3,
                depends_on: 2,
            }, // C -> B
            Dependency {
                task_id: 4,
                depends_on: 3,
            }, // D -> C
        ];

        let result = transitive_dependencies(4, &deps);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&1));
        assert!(result.contains(&2));
        assert!(result.contains(&3));
    }

    #[test]
    fn test_order_conflict() {
        // Task 2 depends on Task 1, but has lower manual_order (conflict)
        let tasks = vec![
            make_task(1, 30.0), // Higher order
            make_task(2, 10.0), // Lower order but depends on 1
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        let conflicts = find_order_conflicts(&tasks, &deps);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0], (2, 10.0, 1, 30.0));
    }

    #[test]
    fn test_no_order_conflict() {
        // Task 2 depends on Task 1, and has higher manual_order (no conflict)
        let tasks = vec![
            make_task(1, 10.0), // Lower order
            make_task(2, 30.0), // Higher order, depends on 1
        ];
        let deps = vec![Dependency {
            task_id: 2,
            depends_on: 1,
        }];

        let conflicts = find_order_conflicts(&tasks, &deps);
        assert!(conflicts.is_empty());
    }
}

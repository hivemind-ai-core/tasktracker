//! Manual ordering system for tasks
//!
//! Implements float-based ordering with gap insertion and precision detection.
//! Per SPEC.md Section 7.2.

use crate::core::models::Task;
use crate::error::{Error, Result};

/// Gap size between default orders
pub const ORDER_GAP: f64 = 10.0;

/// Calculate manual order for a new or repositioned task
///
/// | Scenario | Calculation |
/// |:---------|:------------|
/// | New task, no positioning hint | `MAX(manual_order) + 10.0` (or `10.0` if no tasks) |
/// | Insert between task A and task B | `(A.manual_order + B.manual_order) / 2.0` |
/// | Insert after task A only | `A.manual_order + 10.0` |
/// | Insert before task B only | `B.manual_order - 10.0` |
///
/// Returns error if float precision is exhausted.
pub fn calculate_manual_order(
    tasks: &[Task],
    after_id: Option<i64>,
    before_id: Option<i64>,
) -> Result<f64> {
    match (after_id, before_id) {
        // No positioning hint: append at end
        (None, None) => {
            let max_order = tasks.iter().map(|t| t.manual_order).fold(0.0, f64::max);
            Ok(if max_order == 0.0 {
                ORDER_GAP
            } else {
                max_order + ORDER_GAP
            })
        }

        // Insert after a specific task
        (Some(after), None) => {
            let after_task = find_task(tasks, after)?;
            Ok(after_task.manual_order + ORDER_GAP)
        }

        // Insert before a specific task
        (None, Some(before)) => {
            let before_task = find_task(tasks, before)?;
            Ok(before_task.manual_order - ORDER_GAP)
        }

        // Insert between two tasks
        (Some(after), Some(before)) => {
            let after_task = find_task(tasks, after)?;
            let before_task = find_task(tasks, before)?;

            let a = after_task.manual_order;
            let b = before_task.manual_order;

            // Ensure after < before for the midpoint calculation
            if a >= b {
                return Err(Error::NotSupported(
                    "After task must have lower order than before task".to_string(),
                ));
            }

            let midpoint = (a + b) / 2.0;

            // Check for float precision exhaustion
            if midpoint == a || midpoint == b {
                return Err(Error::FloatPrecisionExhausted);
            }

            Ok(midpoint)
        }
    }
}

/// Find a task by ID
fn find_task(tasks: &[Task], id: i64) -> Result<&Task> {
    tasks
        .iter()
        .find(|t| t.id == id)
        .ok_or(Error::TaskNotFound(id))
}

/// Generate reindexed manual orders
///
/// Returns a vector of (task_id, new_manual_order) pairs.
/// Orders are assigned as 10.0, 20.0, 30.0, ... preserving current sorted order.
pub fn reindex_orders(tasks: &[Task]) -> Vec<(i64, f64)> {
    let mut sorted: Vec<&Task> = tasks.iter().collect();
    sorted.sort_by(|a, b| {
        a.manual_order
            .partial_cmp(&b.manual_order)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted
        .iter()
        .enumerate()
        .map(|(idx, task)| (task.id, (idx + 1) as f64 * ORDER_GAP))
        .collect()
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
    fn test_calculate_order_empty() {
        let tasks = vec![];
        let order = calculate_manual_order(&tasks, None, None).unwrap();
        assert_eq!(order, ORDER_GAP);
    }

    #[test]
    fn test_calculate_order_append() {
        let tasks = vec![make_task(1, 10.0), make_task(2, 20.0)];
        let order = calculate_manual_order(&tasks, None, None).unwrap();
        assert_eq!(order, 30.0);
    }

    #[test]
    fn test_calculate_order_after() {
        let tasks = vec![make_task(1, 10.0)];
        let order = calculate_manual_order(&tasks, Some(1), None).unwrap();
        assert_eq!(order, 20.0);
    }

    #[test]
    fn test_calculate_order_before() {
        let tasks = vec![make_task(1, 30.0)];
        let order = calculate_manual_order(&tasks, None, Some(1)).unwrap();
        assert_eq!(order, 20.0);
    }

    #[test]
    fn test_calculate_order_between() {
        let tasks = vec![make_task(1, 10.0), make_task(2, 20.0)];
        let order = calculate_manual_order(&tasks, Some(1), Some(2)).unwrap();
        assert_eq!(order, 15.0);
    }

    #[test]
    fn test_calculate_order_between_multiple_gaps() {
        // Tasks at 10.0 and 20.0, inserting between should give 15.0
        let tasks = vec![make_task(1, 10.0), make_task(2, 30.0)];
        let order = calculate_manual_order(&tasks, Some(1), Some(2)).unwrap();
        assert_eq!(order, 20.0);
    }

    #[test]
    fn test_calculate_order_task_not_found() {
        let tasks = vec![make_task(1, 10.0)];
        let result = calculate_manual_order(&tasks, Some(999), None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::TaskNotFound(999)));
    }

    #[test]
    fn test_calculate_order_invalid_range() {
        let tasks = vec![make_task(1, 30.0), make_task(2, 10.0)];
        // Trying to insert "after" 1 and "before" 2, but 1's order > 2's order
        let result = calculate_manual_order(&tasks, Some(1), Some(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_float_precision_exhaustion() {
        // Create two tasks very close together
        let tasks = vec![make_task(1, 1.0), make_task(2, 1.0 + f64::EPSILON)];
        // Trying to insert between them should fail due to precision
        let result = calculate_manual_order(&tasks, Some(1), Some(2));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::FloatPrecisionExhausted
        ));
    }

    #[test]
    fn test_reindex_orders() {
        let tasks = vec![make_task(1, 5.5), make_task(2, 1.1), make_task(3, 10.0)];
        let reindexed = reindex_orders(&tasks);

        // Should be sorted by current order: 2 (1.1), 1 (5.5), 3 (10.0)
        assert_eq!(reindexed.len(), 3);
        assert_eq!(reindexed[0], (2, 10.0)); // First gets 10.0
        assert_eq!(reindexed[1], (1, 20.0)); // Second gets 20.0
        assert_eq!(reindexed[2], (3, 30.0)); // Third gets 30.0
    }

    #[test]
    fn test_reindex_empty() {
        let tasks: Vec<Task> = vec![];
        let reindexed = reindex_orders(&tasks);
        assert!(reindexed.is_empty());
    }
}

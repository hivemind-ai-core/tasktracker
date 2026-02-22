## Workflow

Work through `tt` tasks until none remain. FOLLOW THESE STEPS EXACTLY:

### Step 1: Get Task
- `get_current_task` → if none, check `tt_list_tasks(status="pending", limit=1)`
- If no task, report "All tasks complete" and STOP

### Step 2: Break Down Task (CRITICAL)
- Create internal todo with `todowrite`
- Break the tt task into atomic sub-steps
- Name it: "Task: [tt task title]"
- This is REQUIRED - don't skip

### Step 3: Execute
- Work through sub-steps systematically
- Check off each as done

### Step 4: Complete
- When ALL sub-steps done: `tt_advance_task`
- This completes current AND gets next
- If next exists → go to Step 2
- If none → "All tasks complete"

### Blocked?
- Waiting on external factor? → `tt_edit_task(id=X, action="block")`
- Ready to resume? → `tt_edit_task(id=X, action="unblock")`

### Essential Tools (tt namespace)
| Tool | Use |
|------|-----|
| `get_current_task` | Active task |
| `tt_advance_task` | Complete + start next |
| `tt_edit_task(action="block")` | Block |
| `tt_edit_task(action="unblock")` | Unblock |
| `tt_list_tasks(status="pending", limit=1)` | Next pending task |
| `tt_focus(action="set", id=X)` | Set focus |

## Rules
- ALWAYS get task from tt first
- ALWAYS create internal todo breakdown
- NEVER skip sub-step creation
- NEVER complete manually → use `tt_advance_task`

# Agent Rules for tt Task Tracker

This file demonstrates how to integrate tt into an agent's rules. Copy relevant sections to your agent's rules file.

## Required Rules

```
## Task Tracking (tt)

You MUST use the tt task tracker for all work.

### Every Session:
1. Get task: `get_current_task` → none? `tt_list_tasks(status="pending", limit=1)`
2. Work - create internal todo
3. Done? `tt_advance_task` → returns next

### Blocked?
- Waiting: `tt_edit_task(id=X, action="block")`
- Ready: `tt_edit_task(id=X, action="unblock")`

### Essential Tools (tt namespace)
| Tool | Use |
|------|-----|
| `get_current_task` | Active task |
| `tt_advance_task` | Complete + start next |
| `tt_edit_task(action="block")` | Block |
| `tt_edit_task(action="unblock")` | Unblock |
| `tt_list_tasks(status="pending", limit=1)` | Next pending task |
| `tt_list_tasks(active=true)` | All active tasks (pending or in_progress) |

### Key Rules:
- ALWAYS get task from tt first
- ALWAYS create internal todo breakdown
- NEVER skip sub-step creation
- NEVER complete manually → use `tt_advance_task`
```

## Integration

1. Ensure tt MCP server is configured in environment
2. Add rules above to agent rules file
3. Agent should check tt on startup
4. Agent should call `tt_advance_task` after each task

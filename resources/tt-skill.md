# tt - Task Tracker Skill

## Overview
This skill provides commands for working with the tt task tracker. tt manages tasks as nodes in a Directed Acyclic Graph (DAG) with dependencies.

## Commands (tt namespace)

### Get Current Task
- Tool: `get_current_task`
- Returns: The currently active task (in_progress)

### Get Next Task (use list instead)
- Tool: `tt_list_tasks(status="pending", limit=1)`
- Returns: The next task to work on

### Advance Task (COMPLETE + NEXT)
- Tool: `tt_advance_task`
- Use: Call when current task is done - completes current AND starts next
- CRITICAL: Always use this instead of manually completing

### Block/Unblock Task
- Tool: `tt_edit_task` with action="block" or action="unblock"
- Use when: Waiting on external factor

### List Tasks
- Tool: `tt_list_tasks` with status filter or active filter
- Use: `tt_list_tasks(status="pending")` for pending tasks
- Use: `tt_list_tasks(active=true)` for all active tasks (pending or in_progress)

## Required Workflow - ALWAYS FOLLOW THIS:

1. Start of session: Call `get_current_task` → none? use `tt_list_tasks(status="pending", limit=1)`
2. Work on task until done
3. When done: Call `tt_advance_task` (NOT manually complete)
4. Repeat until no pending tasks

## Never:
- Assume what task to work on - ALWAYS use tt tools
- Skip marking complete - ALWAYS use `tt_advance_task`
- Work on tasks not from tt list

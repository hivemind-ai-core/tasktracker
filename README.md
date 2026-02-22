# tt — DAG-Based Task Tracker

A CLI tool and MCP server for managing tasks with dependencies. Tasks are stored as nodes in a Directed Acyclic Graph (DAG), solving the pain of renumbering and reordering markdown files.

Designed for AI coding agents: you set high-level goals ("focus"), and the AI autonomously works through the dependency chain.

## Building

```bash
# Build the binary
cargo build --release

# The binary will be at target/release/tt
```

## Quick Start

### 1. Initialize a project

```bash
tt init
```

This creates a `.tt/` directory with `tt.db` and an `artifacts/` subfolder.

### 2. Create tasks

```bash
tt add "Set up database" "Create PostgreSQL schema" "Schema applied and tested"
tt add "Implement auth" "Add JWT authentication" "Users can log in" --depends-on 1
tt add "Build API" "Create REST endpoints" "All endpoints working" --depends-on 2
```

### 3. Set a focus (optional)

A focus creates a **view** into your task list. When set, `tt list` and `tt next` only show the tasks needed to reach that focus — its transitive dependencies plus itself. Without a focus, all tasks are shown.

```bash
tt focus set 3    # Focus on task #3 (Build API)
tt focus show     # See current focus
tt focus clear    # Clear focus (show all tasks)
tt focus next     # Focus on next incomplete task
tt focus last     # Focus on most recent task
```

### 4. Work through tasks

```bash
tt current        # Show the current active task
tt next           # Show the next task to work on
tt advance        # Complete current + start next in one command
```

## Installing as an MCP Server

### Claude Code

```bash
# Show instructions
tt install --tool claude

# Install locally (current directory)
tt install --tool claude --local

# Install globally
tt install --tool claude --global
```

### Kilo Code

```bash
tt install --tool kilo
tt install --tool kilo --local
tt install --tool kilo --global
```

### Kimi Code

```bash
tt install --tool kimi
tt install --tool kimi --local
tt install --tool kimi --global
```

### Manual setup

Add to your MCP config file:

```json
{
  "mcpServers": {
    "tt": {
      "command": "/path/to/tt",
      "args": ["mcp"]
    }
  }
}
```

## Command Reference

### Task Management

| Command | Description |
|---------|-------------|
| `tt add "title" "desc" "dod"` | Create a task |
| `tt show <id>` | Show task details |
| `tt edit <id> --title "new title"` | Update task fields |
| `tt list` | List tasks in dependency order |
| `tt list --all` | List all tasks (not just focused) |
| `tt list --status pending` | Filter by status |

### Workflow

| Command | Description |
|---------|-------------|
| `tt current` | Show current active task |
| `tt next` | Show next runnable task |
| `tt advance` | Complete current + start next |
| `tt advance --dry-run` | Preview without executing |
| `tt edit <id> --action complete` | Complete a task |
| `tt edit <id> --action cancel` | Cancel a task |
| `tt edit <id> --action block` | Block a task |
| `tt edit <id> --action unblock` | Unblock a task |

### Focus (Targets)

| Command | Description |
|---------|-------------|
| `tt focus set <id>` | Set focus to a task |
| `tt focus show` | Show current focus |
| `tt focus clear` | Clear focus |
| `tt focus next` | Focus on next incomplete task |
| `tt focus last` | Focus on most recent task |

### Dependencies

| Command | Description |
|---------|-------------|
| `tt depend <id> <deps...>` | Add dependencies |
| `tt depend <id> --remove <deps...>` | Remove dependencies |

### Artifacts

| Command | Description |
|---------|-------------|
| `tt log <name> --file <path>` | Attach artifact to active task |
| `tt artifacts` | List artifacts for active task |
| `tt artifacts --task <id>` | List artifacts for a task |

### Ordering

| Command | Description |
|---------|-------------|
| `tt reorder <id> --after <id>` | Move task after another |
| `tt reorder <id> --before <id>` | Move task before another |
| `tt reindex` | Reindex all task orders |

### Advanced

| Command | Description |
|---------|-------------|
| `tt split <id> "title1" "desc1" "dod1" "title2" ...` | Split task into subtasks |
| `tt archive all` | Archive completed/cancelled tasks |
| `tt dump <file>` | Export database to TOML |
| `tt restore <file>` | Import database from TOML |

### MCP Server

| Command | Description |
|---------|-------------|
| `tt mcp` | Start MCP server |
| `tt install --tool <claude\|kilo\|kimi>` | Install MCP in AI tool |

## How Focus Works

When you set a focus, `tt` only considers the transitive dependencies of that task:

```
If you focus on #5 (Build API), and:
  #5 depends on #4 (Write tests)
  #4 depends on #3 (Implement feature)
  #3 depends on #1 (Set up database)

Then tt list/next only shows #1, #3, #4, #5 — ignoring unrelated tasks.
```

Once all tasks in the subgraph are complete, `tt next` reports "Target Reached."

## Status Indicators

| Symbol | Status |
|--------|--------|
| `○` | pending |
| `●` | in_progress |
| `✓` | completed |
| `✗` | blocked |
| `✕` | cancelled |

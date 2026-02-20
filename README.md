# tt — DAG-Based Task Tracker

A CLI tool and MCP server for managing tasks with dependencies. Tasks are stored as nodes in a Directed Acyclic Graph (DAG), solving the pain of renumbering and reordering markdown files.

Designed for AI coding agents: you set high-level goals ("targets"), and the AI autonomously works through the dependency chain.

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

### 3. Set a target (optional)

A target creates a **focus** or **view** into your task list. When set, `tt list` and `tt next` only show the tasks needed to reach that target — its transitive dependencies plus itself. Without a target, all tasks are shown.

```bash
tt target 3    # Target task #3 (Build API)
tt target      # See current target
tt target none # Clear target (show all tasks)
```

### 4. Work through tasks

```bash
tt next        # Show the next task to work on
tt start       # Start the next available task
# ... do work ...
tt advance     # Complete current + start next in one command
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

## Common Commands

| Command | Description |
|---------|-------------|
| `tt add "title" "desc" "dod"` | Create a task |
| `tt list` | List tasks in dependency order |
| `tt next` | Get next runnable task |
| `tt start [id]` | Start a task |
| `tt advance` | Complete current + start next |
| `tt target [id]` | Set/view target |
| `tt depend 2 on 1` | Task #2 depends on #1 |
| `tt log research --file path.md` | Attach artifact to active task |
| `tt show 1` | Show task details |

## How Targets Work

When you set a target, `tt` only considers the transitive dependencies of that task:

```
If you target #5 (Build API), and:
  #5 depends on #4 (Write tests)
  #4 depends on #3 (Implement feature)
  #3 depends on #1 (Set up database)

Then tt list/next only shows #1, #3, #4, #5 — ignoring unrelated tasks.
```

Once all tasks in the subgraph are complete, `tt next` reports "Target Reached."

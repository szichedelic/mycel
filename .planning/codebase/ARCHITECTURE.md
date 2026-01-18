# Architecture

**Analysis Date:** 2026-01-17

## Pattern Overview

**Overall:** Modular CLI/TUI Application with Command Pattern

**Key Characteristics:**
- Dual interface: TUI (default) and CLI subcommands
- Layered architecture with clear separation between UI, business logic, and external system integration
- Persistent state via SQLite database
- External process orchestration (git, tmux) via shell commands
- Async runtime (tokio) with synchronous subsystems

## Layers

**Entry Point (main.rs):**
- Purpose: Parse CLI arguments, route to appropriate handler
- Location: `src/main.rs`
- Contains: Clap argument definitions, command dispatch
- Depends on: All modules
- Used by: Binary execution

**TUI Layer:**
- Purpose: Interactive terminal dashboard for session management
- Location: `src/tui/mod.rs`, `src/tui/logo.rs`
- Contains: Ratatui-based UI, event loop, state management
- Depends on: db, session, worktree, bank, config, disk
- Used by: main.rs (default command)

**CLI Layer:**
- Purpose: Non-interactive command execution
- Location: `src/cli/` (16 subcommand modules)
- Contains: Individual command handlers matching CLI subcommands
- Depends on: db, session, worktree, bank, config, confirm
- Used by: main.rs (subcommand dispatch)

**Database Layer:**
- Purpose: Persistent state storage for projects and sessions
- Location: `src/db/mod.rs`
- Contains: SQLite operations, schema management, CRUD for projects/sessions/history
- Depends on: rusqlite, dirs (for data directory)
- Used by: tui, cli/*

**Session Layer:**
- Purpose: tmux session lifecycle management
- Location: `src/session/mod.rs`
- Contains: SessionManager struct for create/attach/kill/is_alive operations
- Depends on: config (ResolvedBackend)
- Used by: tui, cli/spawn, cli/kill, cli/stop, cli/attach, cli/bank

**Worktree Layer:**
- Purpose: Git worktree lifecycle management
- Location: `src/worktree/mod.rs`
- Contains: Create, remove, get_branch, commit_count, symlink creation
- Depends on: config (ProjectConfig)
- Used by: tui, cli/spawn, cli/kill, cli/bank, cli/unbank, cli/init

**Bank Layer:**
- Purpose: Session archival via git bundles
- Location: `src/bank/mod.rs`
- Contains: Bundle creation/restoration, metadata management, import/export archives
- Depends on: flate2, tar (compression), toml (metadata)
- Used by: tui, cli/bank, cli/unbank, cli/bank_export, cli/bank_import, cli/banked

**Config Layer:**
- Purpose: Configuration loading and backend resolution
- Location: `src/config/mod.rs`
- Contains: GlobalConfig, ProjectConfig, TemplateConfig, BackendConfig structs
- Depends on: toml, serde, dirs
- Used by: All modules needing configuration

**Utility Layers:**
- `src/confirm.rs`: Interactive prompts (y/n, string input, select, multi-select)
- `src/disk/mod.rs`: Directory size calculation, filesystem usage stats
- `src/notify/mod.rs`: macOS desktop notifications via osascript

## Data Flow

**Session Creation (spawn):**

1. CLI/TUI receives spawn request with session name
2. Resolve git root from current directory (`worktree::find_git_root`)
3. Load project from database, load project+global configs
4. Resolve backend (claude/codex/custom) from configs
5. Create git worktree with temp branch (`worktree::create`)
6. Create tmux session running backend command (`session::create`)
7. Persist session record to database (`db.add_session`)
8. Optionally send initial prompt to tmux session

**Session Banking (bank):**

1. Verify no uncommitted changes in worktree
2. Create git bundle of commits since base branch (`bank::create_bundle`)
3. Write metadata TOML alongside bundle
4. Kill tmux session, remove worktree, delete local branch
5. Archive session to history table, remove from active sessions

**State Management:**
- SQLite database at `~/.local/share/mycel/mycel.db` (via dirs::data_dir)
- Tables: projects, sessions, session_history
- Bundles stored at `~/.mycel/bank/{project_name}/{session_name}.bundle`
- Configs: `~/.mycel/config.toml` (global), `.mycel.toml` or `~/.config/mycel/projects/{name}.toml` (project)

## Key Abstractions

**Database:**
- Purpose: Central state repository
- Examples: `src/db/mod.rs`
- Pattern: Repository pattern with typed query methods

**SessionManager:**
- Purpose: tmux process orchestration
- Examples: `src/session/mod.rs`
- Pattern: Stateless service (unit struct with methods)

**Configuration Structs:**
- Purpose: Typed configuration with defaults and overrides
- Examples: `src/config/mod.rs` (GlobalConfig, ProjectConfig, BackendConfig)
- Pattern: Serde-derived structs with Default implementations

**ResolvedBackend:**
- Purpose: Computed backend command from layered config sources
- Examples: `src/config/mod.rs`
- Pattern: Config resolution with override chain (default -> global -> project -> CLI arg)

## Entry Points

**TUI (default):**
- Location: `src/tui/mod.rs::run()`
- Triggers: Running `mycel` with no subcommand
- Responsibilities: Event loop, render dashboard, handle keyboard/mouse input

**CLI Commands:**
- Location: `src/cli/*.rs::run()`
- Triggers: Running `mycel <subcommand>`
- Responsibilities: Execute single operation, print output, exit

**Web Server:**
- Location: `src/cli/web.rs::run()`
- Triggers: Running `mycel web`
- Responsibilities: HTTP server with WebSocket PTY for remote TUI access

## Error Handling

**Strategy:** Result-based with anyhow for ergonomic error propagation

**Patterns:**
- All public functions return `Result<T>` (anyhow::Result)
- Context added via `.context()` or `.with_context()`
- Early returns with `bail!()` for validation failures
- Warnings printed to stderr for non-fatal issues (e.g., symlink failures)

## Cross-Cutting Concerns

**Logging:** tracing + tracing_subscriber (initialized in main, minimal use in codebase)

**Validation:**
- Project must be registered before session operations
- Session names must be unique per project
- Worktrees must have no uncommitted changes before banking

**Authentication:** Token-based for web server (optional --token flag)

---

*Architecture analysis: 2026-01-17*

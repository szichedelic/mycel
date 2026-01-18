# Codebase Structure

**Analysis Date:** 2026-01-17

## Directory Layout

```
mycel/
├── src/                    # All Rust source code
│   ├── main.rs             # CLI entry point with clap definitions
│   ├── confirm.rs          # Interactive prompt utilities
│   ├── cli/                # CLI subcommand handlers
│   ├── tui/                # TUI dashboard implementation
│   ├── db/                 # SQLite database layer
│   ├── session/            # tmux session management
│   ├── worktree/           # Git worktree operations
│   ├── bank/               # Git bundle banking system
│   ├── config/             # Configuration parsing
│   ├── disk/               # Disk usage utilities
│   └── notify/             # Desktop notifications
├── Cargo.toml              # Rust package manifest
├── Cargo.lock              # Dependency lockfile
├── .claude/                # Claude Code configuration
│   ├── CLAUDE.md           # Project instructions
│   ├── commands/           # Claude slash commands
│   └── skills/             # Claude skills
├── .codex/                 # OpenAI Codex configuration
├── docs/                   # Documentation
│   └── plans/              # Planning documents
├── keys/                   # TLS keys for web server
├── .planning/              # GSD planning artifacts
└── target/                 # Build output (gitignored)
```

## Directory Purposes

**src/cli/:**
- Purpose: One module per CLI subcommand
- Contains: 16 async `run()` functions matching Commands enum
- Key files:
  - `mod.rs`: Module re-exports
  - `spawn.rs`: Create new session
  - `init.rs`: Register project with setup wizard
  - `bank.rs`: Archive session to bundle
  - `unbank.rs`: Restore session from bundle
  - `web.rs`: HTTP/WebSocket server for remote TUI

**src/tui/:**
- Purpose: Interactive terminal dashboard
- Contains: Main event loop, rendering, state management
- Key files:
  - `mod.rs`: ~2600 line TUI implementation (App struct, event handling, rendering)
  - `logo.rs`: ASCII art logo generation

**src/db/:**
- Purpose: SQLite persistence layer
- Contains: Database struct, schema migrations, typed queries
- Key files:
  - `mod.rs`: Project/Session/SessionHistory types, CRUD operations

**src/session/:**
- Purpose: tmux process management
- Contains: SessionManager struct for tmux lifecycle
- Key files:
  - `mod.rs`: create, attach, kill, is_alive, send_prompt methods

**src/worktree/:**
- Purpose: Git worktree lifecycle
- Contains: Create/remove worktrees, branch operations, symlink setup
- Key files:
  - `mod.rs`: create, create_from_existing, remove, get_branch, commit_count

**src/bank/:**
- Purpose: Session archival via git bundles
- Contains: Bundle creation/restoration, tar.gz export/import
- Key files:
  - `mod.rs`: BankMetadata, BankedItem types, bundle operations

**src/config/:**
- Purpose: TOML configuration parsing
- Contains: GlobalConfig, ProjectConfig, TemplateConfig, BackendConfig
- Key files:
  - `mod.rs`: Config loading, backend resolution, default values

**src/disk/:**
- Purpose: Disk space utilities
- Contains: Directory size calculation, statvfs wrapper
- Key files:
  - `mod.rs`: dir_size_bytes, filesystem_usage (Unix-specific)

**src/notify/:**
- Purpose: Desktop notifications
- Contains: macOS osascript notification sender
- Key files:
  - `mod.rs`: send_notification (macOS only, no-op on other platforms)

## Key File Locations

**Entry Points:**
- `src/main.rs`: Binary entry, CLI parsing, command dispatch

**Configuration:**
- `Cargo.toml`: Dependencies and build settings
- `.claude/CLAUDE.md`: Claude Code project instructions
- `~/.mycel/config.toml`: Global user config (runtime)
- `.mycel.toml`: Project-local config (runtime)
- `~/.config/mycel/projects/{name}.toml`: External project config (runtime)

**Core Logic:**
- `src/tui/mod.rs`: Main TUI implementation (~2600 lines)
- `src/db/mod.rs`: Database operations (~380 lines)
- `src/worktree/mod.rs`: Git worktree operations (~290 lines)
- `src/bank/mod.rs`: Banking operations (~335 lines)
- `src/config/mod.rs`: Config structs and resolution (~270 lines)

**Testing:**
- No test files present in codebase

## Naming Conventions

**Files:**
- `mod.rs`: Module entry point (one per directory)
- `{command}.rs`: CLI subcommand matching clap enum variant (lowercase)
- `{feature}.rs`: Standalone utility module

**Directories:**
- `src/{layer}/`: Feature-based modules (lowercase, singular)

**Rust Naming:**
- Structs: PascalCase (e.g., `SessionManager`, `ProjectConfig`)
- Functions: snake_case (e.g., `find_git_root`, `create_bundle`)
- Constants: SCREAMING_SNAKE_CASE (e.g., `PREVIEW_LINES`, `DOUBLE_CLICK_WINDOW_MS`)

## Where to Add New Code

**New CLI Command:**
1. Add variant to `Commands` enum in `src/main.rs`
2. Create `src/cli/{command}.rs` with `pub async fn run(...) -> Result<()>`
3. Add `pub mod {command};` to `src/cli/mod.rs`
4. Add dispatch case in `main()` match statement

**New Core Feature:**
1. Create `src/{feature}/mod.rs`
2. Add `mod {feature};` to `src/main.rs`
3. Import and use from cli/tui modules

**TUI Feature:**
- Add methods to `App` struct in `src/tui/mod.rs`
- Add keyboard handling in event loop
- Add rendering in appropriate draw function

**Database Schema Change:**
1. Add migration in `Database::init_schema()` in `src/db/mod.rs`
2. Add `ensure_{column}()` migration helper for backwards compatibility
3. Update relevant struct definitions and query methods

**New Configuration Option:**
1. Add field to `GlobalConfig` or `ProjectConfig` in `src/config/mod.rs`
2. Add `#[serde(default)]` or default function if optional
3. Update `Default` impl if needed

## Special Directories

**.claude/:**
- Purpose: Claude Code AI assistant configuration
- Generated: No (manually created)
- Committed: Yes

**.codex/:**
- Purpose: OpenAI Codex configuration
- Generated: No (manually created)
- Committed: Yes

**target/:**
- Purpose: Rust build output
- Generated: Yes (by cargo)
- Committed: No (in .gitignore)

**keys/:**
- Purpose: TLS certificates for web server
- Generated: Manually or via certbot
- Committed: Partial (example keys only)

**.planning/:**
- Purpose: GSD planning documents
- Generated: By GSD commands
- Committed: Yes

---

*Structure analysis: 2026-01-17*

# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Mycel is a Rust CLI/TUI for managing parallel AI coding sessions (Claude, Codex) across Git worktrees. It tracks session state in SQLite, launches backends in tmux/Docker/remote hosts, and provides a terminal dashboard.

## Development

```bash
cargo build --release    # Build optimized binary
cargo build              # Debug build
cargo check              # Type-check without building
cargo clippy             # Lint
cargo test               # Run inline unit tests
mycel                    # Run TUI (if symlinked to /usr/local/bin)
```

Tests are defined inline in `#[cfg(test)]` modules; run `cargo test` before submitting changes. The project has no CI pipeline.

## Skills

- `/work-on-issue` - Pick a GitHub issue and implement it with organized commits

## Commit Conventions

- Single line messages, no co-authorship
- Format: `{scope}: {description}`
- Scopes: tui, db, session, bank, worktree, config, fix, refactor

## Architecture

**Entry point**: `src/main.rs` defines the CLI via clap derive. Running without a subcommand launches the TUI (`tui::run()`). Each subcommand dispatches to a handler in `src/cli/`.

**Runtime abstraction**: Sessions are backed by a `RuntimeProvider` trait (`src/session/runtime.rs`) with three implementations:
- `TmuxProvider` (`session/tmux.rs`) - default, most stable
- `ComposeProvider` (`session/compose.rs`) - local Docker Compose
- `RemoteProvider` (`session/remote.rs`) - Docker over SSH

`SessionManager` (`session/mod.rs`) wraps a boxed `RuntimeProvider` and is the main interface for starting/stopping/querying sessions.

**Database**: `src/db/mod.rs` contains all SQLite logic in a single file — schema migrations, CRUD for projects, sessions, session_history, session_runtimes, session_services, and hosts. Uses rusqlite directly (no ORM).

**TUI**: `src/tui/mod.rs` is a single large file containing the full Ratatui application — `App` struct, state management, rendering, and input handling. It has multiple view modes: sessions (default), history, hosts, idle-runtime review. Spawning and handoff flows use inline multi-step input collection.

**Banking**: `src/bank/mod.rs` handles git-bundle-based session archival. Sessions can be banked (bundled + archived), unbanked (restored), and exported/imported as `.tar.gz` archives.

**Config**: `src/config/mod.rs` parses TOML config from global (`~/.mycel/config.toml` or `~/.mycelrc`) and project-level (external or `.mycel.toml`) sources. Manages backend resolution, templates, and symlink paths.

**Worktree**: `src/worktree/mod.rs` wraps git worktree operations (create, remove, list) and handles symlink setup for new worktrees.

## Design Principles

- **TUI-first**: All features should be accessible from TUI, CLI is secondary
- **Single-file modules**: Most modules are a single `mod.rs` rather than split across files
- **No async in core logic**: Async is used at the top level (tokio) but most internal code is synchronous, shelling out via `std::process::Command`

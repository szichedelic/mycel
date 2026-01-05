# Mycel - Claude Code Session Manager

Rust CLI/TUI for managing parallel Claude Code sessions via git worktrees.

## Project Structure

- `src/main.rs` - CLI entry point with clap commands
- `src/tui/` - Ratatui TUI dashboard
- `src/session/` - tmux session management
- `src/worktree/` - Git worktree operations
- `src/bank/` - Git bundle banking for stashing work
- `src/db/` - SQLite database for state
- `src/config/` - TOML configuration parsing

## Development

```bash
cargo build --release    # Build
mycel                    # Run TUI (symlinked to /usr/local/bin)
```

## Skills

- `/work-on-issue` - Pick a GitHub issue and implement it with organized commits

## Commit Conventions

- Single line messages, no co-authorship
- Format: `{scope}: {description}`
- Scopes: tui, db, session, bank, worktree, config, fix, refactor

## Design Principles

- **TUI-first**: All features should be accessible from TUI, CLI is secondary
- **Performance**: Rust + minimal dependencies
- **Simplicity**: Avoid over-engineering

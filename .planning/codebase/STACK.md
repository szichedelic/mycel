# Technology Stack

**Analysis Date:** 2026-01-17

## Languages

**Primary:**
- Rust 2021 edition - All application code

**Secondary:**
- JavaScript (ES6) - Embedded web terminal UI in `src/cli/web.rs`
- AppleScript - macOS notifications via osascript in `src/notify/mod.rs`

## Runtime

**Environment:**
- Rust 1.88.0+ (stable)
- tmux (required for session management)
- git (required for worktree operations)

**Package Manager:**
- Cargo 1.88.0+
- Lockfile: `Cargo.lock` (64KB, committed)

## Frameworks

**Core:**
- clap 4.x - CLI argument parsing with derive macros (`src/main.rs`)
- tokio 1.x (full features) - Async runtime for all operations
- ratatui 0.29 - Terminal UI framework (`src/tui/mod.rs`)
- crossterm 0.28 - Terminal backend for ratatui

**Web/API:**
- axum 0.6 (with WebSocket support) - Web server for `mycel web` command
- axum-server 0.5 (with TLS rustls) - HTTPS support for web UI
- portable-pty 0.8 - Pseudo-terminal for web-based TUI

**Database:**
- rusqlite 0.32 (bundled SQLite) - Session and project state storage

**Serialization:**
- serde 1.x (derive) - Serialization framework
- serde_json 1.x - JSON for WebSocket messages
- toml 0.8 - Configuration file parsing

**Utilities:**
- anyhow 1.x - Error handling with context
- thiserror 2.x - Custom error types
- tracing 0.1 + tracing-subscriber 0.3 - Logging
- fuzzy-matcher 0.3 - Fuzzy search in TUI
- dirs 5.x - Platform-specific directory resolution
- flate2 1.x - Gzip compression for bank exports
- tar 0.4 - Archive creation for bank exports
- glob 0.3 - File pattern matching for symlinks
- libc 0.2 - Unix filesystem operations (statvfs)
- futures-util 0.3 - Async stream utilities

## Key Dependencies

**Critical:**
- `rusqlite` (bundled) - SQLite compiled into binary, no external dependency
- `ratatui` + `crossterm` - Core TUI rendering stack
- `axum` + `tokio` - Async web server for remote access

**Infrastructure:**
- `portable-pty` - Cross-platform PTY for spawning mycel in web terminal
- `rustls` (via axum-server) - TLS without OpenSSL dependency

## Build Configuration

**Release Profile (`Cargo.toml`):**
```toml
[profile.release]
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit for optimization
strip = true         # Strip symbols
panic = "abort"      # Abort on panic (smaller binary)
```

**Features:**
- `clap/derive` - Procedural macros for CLI
- `tokio/full` - All tokio features
- `axum/ws` - WebSocket support
- `axum-server/tls-rustls` - TLS with rustls
- `rusqlite/bundled` - Self-contained SQLite

## Configuration

**Global Config:**
- Location: `~/.mycel/config.toml` or `~/.mycelrc`
- Managed by: `src/config/mod.rs`
- Format: TOML via `GlobalConfig` struct

**Project Config:**
- Location: `.mycel.toml` in project root OR `~/.config/mycel/projects/{name}.toml`
- Managed by: `src/config/mod.rs`
- Format: TOML via `ProjectConfig` struct

**Database:**
- Location: `{data_dir}/mycel/mycel.db` (platform-specific data directory)
- Managed by: `src/db/mod.rs`
- Schema: projects, sessions, session_history tables

**Bank Storage:**
- Location: `~/.mycel/bank/{project}/`
- Format: Git bundles (`.bundle`) + TOML metadata (`.metadata.toml`)
- Archives: Gzipped tar for export/import

## Platform Requirements

**Development:**
- Rust toolchain (rustup recommended)
- tmux installed and in PATH
- git installed and in PATH

**Production/Runtime:**
- macOS, Linux, or Windows (macOS primary target)
- tmux for session management
- git for worktree operations
- Optional: TLS certificates for HTTPS web UI

**Platform-Specific:**
- macOS: osascript for desktop notifications
- Unix: libc for filesystem stats (statvfs)
- Windows: Symlink support with elevated privileges

---

*Stack analysis: 2026-01-17*

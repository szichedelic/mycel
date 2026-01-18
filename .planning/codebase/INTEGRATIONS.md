# External Integrations

**Analysis Date:** 2026-01-17

## APIs & External Services

**None** - Mycel is a fully local CLI/TUI tool with no external API integrations.

## Data Storage

**Databases:**
- SQLite (bundled via rusqlite)
  - Location: `{platform_data_dir}/mycel/mycel.db`
  - Client: `rusqlite` with bundled SQLite
  - Schema managed in `src/db/mod.rs`

**File Storage:**
- Local filesystem only
  - Git worktrees: `{project}/../.mycel-worktrees/{project}/{session-id}/`
  - Bank bundles: `~/.mycel/bank/{project}/*.bundle`
  - Config: `~/.mycel/config.toml`, `~/.config/mycel/projects/`

**Caching:**
- None - All state is persisted to SQLite or filesystem

## Authentication & Identity

**Auth Provider:**
- None for core functionality
- Optional access token for web UI (`--token` flag on `mycel web`)

**Implementation:**
- Token passed via query parameter to WebSocket endpoint
- Validated in `src/cli/web.rs` before establishing connection

## External CLI Dependencies

**Required:**
| Tool | Purpose | Used In |
|------|---------|---------|
| `tmux` | Session management (create/attach/kill) | `src/session/mod.rs` |
| `git` | Worktree ops, bundling, branch management | `src/worktree/mod.rs`, `src/bank/mod.rs` |

**Optional:**
| Tool | Purpose | Used In |
|------|---------|---------|
| `osascript` | macOS desktop notifications | `src/notify/mod.rs` |
| `claude` | Default AI backend | Launched in tmux session |
| `codex` | Alternative AI backend | Launched in tmux session |

## AI Backends

**Configuration:**
- Global: `~/.mycel/config.toml` under `[backends]`
- Project: `.mycel.toml` under `[backends]`
- Override: `--backend` flag on `mycel spawn`

**Default Backends (built-in):**
```toml
[backends.claude]
command = "claude"

[backends.codex]
command = "codex"
```

**Custom Backend Example:**
```toml
[backends.aider]
command = "aider"
args = ["--model", "gpt-4"]
```

**Resolution:** `src/config/mod.rs::resolve_backend()`

## Monitoring & Observability

**Error Tracking:**
- None - Errors logged to stderr via tracing

**Logs:**
- tracing-subscriber with default fmt layer
- Initialized in `src/main.rs`

## CI/CD & Deployment

**Hosting:**
- Local binary - no hosted service

**CI Pipeline:**
- Not detected in repository

**Distribution:**
- Manual: `cargo build --release`
- Symlink recommended: `/usr/local/bin/mycel`

## Web UI Integration

**Technology:**
- Self-hosted web server via axum
- xterm.js terminal emulator (CDN: unpkg.com)
- WebSocket for bidirectional PTY communication

**Endpoints:**
- `GET /` - HTML page with terminal
- `GET /ws` - WebSocket for PTY I/O

**TLS Configuration:**
- Optional via `--tls-cert` and `--tls-key` flags
- Uses rustls (no OpenSSL dependency)
- Sample keys in `keys/` directory (gitignored)

## Environment Configuration

**Required env vars:**
- None - All config via files or CLI flags

**Optional env vars:**
- Standard Rust/Cargo variables
- `TERM` set to `xterm-256color` for web PTY sessions

**Secrets location:**
- TLS keys: `keys/` directory (gitignored)
- Access token: Passed via CLI flag, not persisted

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

## Desktop Integration

**macOS:**
- Notifications via osascript (AppleScript)
- Triggered by `mycel notify` command polling for session events

**Linux/Windows:**
- Notification support not implemented (no-op)

## Git Operations

**Commands used:**
| Command | Purpose | Location |
|---------|---------|----------|
| `git rev-parse --show-toplevel` | Find repo root | `src/worktree/mod.rs` |
| `git worktree add` | Create worktree | `src/worktree/mod.rs` |
| `git worktree remove` | Delete worktree | `src/worktree/mod.rs` |
| `git rev-parse --abbrev-ref HEAD` | Get branch name | `src/worktree/mod.rs` |
| `git branch -D` | Delete branch | `src/worktree/mod.rs` |
| `git rev-list --count` | Count commits | `src/worktree/mod.rs`, `src/bank/mod.rs` |
| `git bundle create` | Create bundle | `src/bank/mod.rs` |
| `git bundle verify` | Verify bundle | `src/bank/mod.rs` |
| `git fetch {bundle}` | Restore from bundle | `src/bank/mod.rs` |

## tmux Operations

**Commands used:**
| Command | Purpose | Location |
|---------|---------|----------|
| `tmux new-session -d` | Create detached session | `src/session/mod.rs` |
| `tmux send-keys` | Send commands to session | `src/session/mod.rs` |
| `tmux has-session` | Check session alive | `src/session/mod.rs` |
| `tmux attach-session` | Attach to session | `src/session/mod.rs` |
| `tmux kill-session` | Kill session | `src/session/mod.rs` |

---

*Integration audit: 2026-01-17*

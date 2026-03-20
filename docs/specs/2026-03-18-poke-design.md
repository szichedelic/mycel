# Poke — Agent Attention Aggregator

**Date**: 2026-03-18
**Status**: Draft
**Repo**: Separate repo (`poke`)

## Problem

When running multiple AI coding agents (Claude Code, Codex) across tmux sessions — including happy-wrapped and remote-controlled sessions — agents frequently block waiting for human input (questions, approvals, choices). There's no unified way to know which agents need attention or to quickly switch to them.

## Solution

`poke` is a standalone Rust CLI that detects AI agents waiting for human input across tmux sessions, aggregates their status, and lets you quickly switch to the relevant pane. It integrates passively into the tmux status bar for ambient awareness.

## Design Principles

- **Structured-first, scraping fallback**: Use agent hooks (Claude Code) for reliable detection, fall back to tmux pane scraping for agents without hook support
- **Zero overhead when idle**: No persistent daemon — status line script runs on interval, watch mode only when explicitly launched
- **Tmux-native switching**: Leverage `tmux switch-client` for instant pane jumping
- **Standalone**: Works independently of mycel; mycel integrates as a consumer later

## Modes of Operation

### Status Line (`poke count`)
Outputs a short string for tmux status bar embedding. Shows `● 3` when agents are waiting, empty when none. Must run in <100ms.

**Error handling**: On any failure (tmux not running, `~/.poke/` missing, scan error), `poke count` outputs an empty string silently. It must never print errors to stdout as this would corrupt the tmux status bar.

```
# .tmux.conf
set -g status-right "#(poke count)"
```

### CLI (`poke` / `poke list`)
Scans and prints a table of waiting agents with context. Interactive selection switches to the chosen pane.

```
 # │ Agent       │ Waiting For │ Context                          │ Since
───┼─────────────┼─────────────┼──────────────────────────────────┼────────
 1 │ claude-code │ approval    │ wants to edit src/main.rs        │ 3m ago
 2 │ claude-code │ question    │ "should I split this into two…"  │ 8m ago
 3 │ codex       │ choice      │ pick an option (1/2/3)           │ 12m ago

Select [1-3, q to quit]:
```

When stdout is not a TTY (piped or scripted), `poke list` skips the interactive prompt and outputs the table only. A `--json` flag outputs machine-readable JSON for scripting.

### Watch (`poke watch`)
Ratatui TUI that live-updates every 2-3 seconds. Arrow keys + Enter to select and switch, `q` to quit.

## Detection System

### Detector Trait
```rust
trait Detector {
    fn scan(&self) -> Vec<AgentStatus>;
}
```

Both backends implement this. The aggregator runs both, deduplicates, and returns a unified list.

### Deduplication & Aggregation

The dedup key is `tmux_pane` (e.g., `%42`) — each pane can have at most one agent. When both detectors find the same pane:
- **Structured wins**: its data is more reliable (agent-reported summary, accurate waiting_type)
- **Scraping result is discarded** for that pane

The aggregator filters out any entries with `status: "working"` — only `"waiting"` entries appear in output.

### Structured Detection (Claude Code)
Claude Code supports a `notification` hook that fires when the agent needs user attention. `poke init` adds a hook entry to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "notification": [{
      "type": "command",
      "command": "poke hook-notify"
    }]
  }
}
```

`poke init` is idempotent — it reads the existing settings, adds the hook only if not already present, and preserves all other configuration.

#### `poke hook-notify` Interface

The hook receives Claude Code's notification context as JSON on stdin. It extracts the relevant fields and discovers tmux context from the environment:

- **Tmux pane**: read from `$TMUX_PANE` environment variable (always set inside tmux)
- **Tmux session**: resolved via `tmux display-message -p '#S'`
- **Session ID**: derived from `$TMUX_PANE` (the pane ID like `%42` is unique and stable)

It writes an event file to `~/.poke/events/{pane_id}.json`:

```json
{
  "agent": "claude-code",
  "status": "waiting",
  "waiting_type": "question",
  "summary": "Should I create a new file for the test?",
  "tmux_session": "mycel-feature-x",
  "tmux_pane": "%42",
  "pid": 12345,
  "since": "2026-03-18T14:23:00Z"
}
```

#### Event Lifecycle

When the agent resumes (user responds), a subsequent hook invocation overwrites the file with `"status": "working"`. The canonical lifecycle is:

1. Agent needs input → hook writes `status: "waiting"`
2. User responds → hook writes `status: "working"`
3. Agent finishes or user exits → no hook fires, file becomes stale

**Stale cleanup**: `poke` checks the `pid` field against running processes. If the process is gone, the event file is deleted. This runs on every scan. The `stale_timeout_secs` config is a secondary fallback for cases where the PID was recycled — files older than this threshold are deleted regardless.

### Scraping Detection (Codex, ad-hoc agents)
For agents without hook support:

1. `tmux list-panes -a` to enumerate all panes
2. `tmux capture-pane` to grab last ~20 lines of each
3. Pattern match against known prompt signatures from `~/.poke/patterns.toml`
4. Classify waiting type (question/approval/choice) based on the match

Default patterns ship with the binary. Users extend via config.

**Context for scraped detections**: The `summary`/context field is populated from the matched line(s) in the pane buffer, truncated to ~80 chars.

**`since` for scraped detections**: Scraping is stateless — it reports the current scan time as `since`. There is no persistence of "first seen" across scans. This is an acceptable trade-off; structured detection provides accurate timing, and scraping is the fallback path.

**False positive tolerance**: Scraping patterns (especially generic ones like `\?\s*$`) will produce false positives. This is expected and acceptable — structured detection is the primary path. The scraper is a best-effort fallback. Users can tune patterns or exclude noisy sessions via config.

## Switching

- **Same tmux server**: `tmux switch-client -t {session}:{window}.{pane}`
- **Remote/happy sessions**: Switch to the tmux pane containing the remote connection — poke doesn't need SSH awareness, it just targets the local pane

## Configuration

### Directory Structure
```
~/.poke/
├── config.toml          # User configuration
├── events/              # Structured status files from hooks
│   └── {session_id}.json
└── patterns.toml        # Agent prompt patterns for scraping
```

### config.toml
```toml
# Refresh interval for `poke watch` TUI mode (seconds)
scan_interval_secs = 3

# Fallback stale cleanup: delete event files older than this even if PID check is inconclusive
stale_timeout_secs = 300

status_format = "● {count}"
status_empty = ""

[detectors]
structured = true
scraping = true

[scraping]
include_sessions = []
exclude_sessions = ["scratch", "music"]
```

### patterns.toml
```toml
[[patterns]]
name = "claude-code-approval"
agent = "claude-code"
waiting_type = "approval"
regex = '(Allow|Approve|permit).*\?(.*\(Y/n\)|\[y/N\])'

[[patterns]]
name = "claude-code-question"
agent = "claude-code"
waiting_type = "question"
regex = '\?\s*$'

[[patterns]]
name = "codex-choice"
agent = "codex"
waiting_type = "choice"
regex = '^\s*\d+[\.\)]\s+'
```

## Project Structure

```
poke/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry (clap)
│   ├── detect/
│   │   ├── mod.rs         # Detector trait, aggregator, dedup
│   │   ├── structured.rs  # Event file reader
│   │   └── scraper.rs     # Tmux pane scraper + pattern engine
│   ├── switch.rs          # Tmux switch-client logic
│   ├── config.rs          # Config + patterns loading
│   ├── models.rs          # AgentStatus, WaitingType, etc.
│   ├── display/
│   │   ├── mod.rs         # Shared formatting
│   │   ├── cli.rs         # Table output + interactive select
│   │   └── tui.rs         # Ratatui watch mode
│   └── hook.rs            # hook-notify subcommand (stdin → event file)
├── defaults/
│   └── patterns.toml      # Default patterns shipped with binary
└── README.md
```

## Implementation Phases

### Phase 1 — Core Detection + CLI
- Models, config loading, Detector trait
- Tmux scraper with default patterns
- `poke list` CLI with interactive select
- `tmux switch-client` switching

### Phase 2 — Structured Detection + Hooks
- Event file reader
- `poke hook-notify` subcommand
- `poke init` to configure Claude Code hooks
- Aggregator merging both detectors with dedup

### Phase 3 — Status Line + Watch Mode
- `poke count` for tmux status bar
- `poke watch` Ratatui TUI
- Stale event cleanup

### Phase 4 — Mycel Integration
- Mycel consumes poke's detection (library crate or CLI output)
- Agent status indicators in mycel's TUI
- Switch-to-agent action from within mycel

### Phase 5 — Quick Response (stretch)
- Respond to simple prompts via `tmux send-keys` without full switching
- Safety guards for unambiguous prompt types only

## Key Dependencies

- **clap**: CLI argument parsing
- **ratatui** + **crossterm**: TUI for watch mode
- **serde** + **serde_json**: Event file serialization
- **toml**: Config file parsing
- **regex**: Pattern matching (Rust `regex` crate — no PCRE; patterns must use Rust regex syntax)
- **chrono**: Timestamp handling

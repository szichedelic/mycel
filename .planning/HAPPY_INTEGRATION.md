# Mycel ↔ Happy Integration Design

**Date:** 2026-01-22
**Status:** Draft

## Overview

Integrate Happy (mobile-first AI session controller) with Mycel (git worktree session manager) to enable managing Claude Code sessions from a phone.

## Problem

Currently, Mycel sessions can only be controlled from the terminal. Users want to:
- Start sessions from their phone
- Monitor session progress while away from computer
- Approve Claude Code permissions remotely
- Switch between sessions from mobile

## Happy Ecosystem

| Component | Purpose |
|-----------|---------|
| `happy-cli` | Wraps `claude`/`codex`, shows QR code for mobile pairing |
| `happy-server` | E2E encrypted relay between CLI and mobile |
| Happy App | iOS/Android/Web client for remote control |
| `@slopus/sync` | State sync library with optimistic updates |

## Relevant Issues

### Mycel Issues (szichedelic/mycel)

| # | Title | Relevance |
|---|-------|-----------|
| 68 | Remote host sessions via SSH | Remote access pattern |
| 51 | Public API (REST/gRPC) | API for mobile clients |
| 50 | Server/daemon mode | Background session management |
| 104 (Happy) | Project directory registry for mobile session spawning | Mobile-first spawn |
| 78 | Webhook notifications | Event notifications to Happy |

### Happy Issues (slopus/happy-cli)

| # | Title | Impact |
|---|-------|--------|
| 405 | Sync between phone and computer | Cross-device session sync |
| 104 | Project directory registry | Mobile spawn without terminal |
| 100 | Push notifications lack context | Need session/project context |
| 87 | --resume not working | Session continuity |
| 64 | Yolo mode not working | Permission bypass in remote |

## Integration Phases

### Phase 1: `--happy` Flag (Minimal Integration)

Add `--happy` flag to `mycel spawn` that wraps the backend with `happy` instead of running directly.

```bash
# Before
mycel spawn feature-x --backend claude
# Runs: claude

# After
mycel spawn feature-x --backend claude --happy
# Runs: happy
```

**Changes:**
- `src/main.rs` - Add `--happy` flag to Spawn command
- `src/cli/spawn.rs` - Pass flag through to session creation
- `src/session/mod.rs` - Detect happy flag, modify command

**Benefits:**
- Instant mobile access to any Mycel session
- No Happy code changes needed
- User scans QR code once per session

### Phase 2: Happy Backend Type

Register `happy` as a first-class backend in config:

```toml
# ~/.mycel/config.toml
[backends.happy]
command = "happy"
args = []
```

Then use it explicitly:
```bash
mycel spawn feature-x --backend happy
```

Or as project default:
```toml
# .mycel.toml
backend = "happy"
```

### Phase 3: State Sync (Future)

Expose Mycel session state to Happy's sync protocol:
- Session list with statuses
- Worktree branch info
- Active/stopped states

Would require:
- Mycel web server enhancement (`mycel web`)
- Happy client-side integration
- WebSocket protocol for real-time updates

### Phase 4: Remote Operations (Future)

Enable Happy app to trigger Mycel operations:
- `spawn` - Create new session
- `attach` - Switch to session
- `bank` - Stash completed work
- `kill` - Clean up session

Requires:
- Mycel daemon mode (#50)
- API layer (#51)
- Auth token system

## Phase 1 Implementation

### CLI Changes

```rust
// src/main.rs
Spawn {
    name: String,
    #[arg(short, long)]
    note: Option<String>,
    #[arg(short, long)]
    template: Option<String>,
    #[arg(short, long)]
    backend: Option<String>,
    /// Wrap backend with Happy for mobile access
    #[arg(long)]
    happy: bool,
}
```

### Backend Resolution

When `--happy` is set:
1. Resolve the backend normally (claude, codex, etc.)
2. Transform: `happy {backend} [args...]`

```rust
fn apply_happy_wrapper(backend: &mut ResolvedBackend) {
    let original_command = std::mem::replace(&mut backend.command, "happy".to_string());
    let original_args = std::mem::take(&mut backend.args);

    // happy {original_command} {original_args...}
    backend.args = vec![original_command];
    backend.args.extend(original_args);
}
```

### User Flow

```
$ mycel spawn auth-feature --happy

Creating worktree...
Starting happy-wrapped claude session...

# In tmux, happy launches with QR code
# User scans QR on phone
# Session now controllable from mobile
```

## Config Option (Phase 2)

```toml
# .mycel.toml
[project]
# Default to happy-wrapped sessions
happy = true

# Or per-template
[templates.mobile-feature]
happy = true
prompt = "Implement the feature described in the issue"
```

## Considerations

### Happy Installation

Check if `happy` is installed before using:
```rust
fn check_happy_available() -> bool {
    Command::new("which")
        .arg("happy")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

If not installed, show helpful error:
```
Happy CLI not found. Install with: npm install -g happy-coder
```

### Session Resume

Happy's `--resume` flag currently has issues (Happy #87). Document workaround: kill/respawn session if mobile sync breaks.

### Permission Mode

Happy respects `--permission-mode` flag. Consider adding to Mycel spawn:
```bash
mycel spawn feature-x --happy --yolo  # bypass permissions
```

Maps to: `happy --permission-mode=auto`

## Success Criteria

Phase 1:
- [x] `--happy` flag added to spawn command
- [x] Backend properly wrapped with happy command
- [ ] Error message if happy not installed
- [ ] Documentation updated

Phase 2:
- [ ] `happy` as native backend type
- [ ] Config option for default happy wrapping
- [ ] Per-template happy setting

## Testing

Manual test flow:
1. `mycel spawn test-happy --happy`
2. Verify QR code appears in tmux session
3. Scan with Happy mobile app
4. Send command from phone
5. Verify command executes in session
6. `mycel kill test-happy --remove`

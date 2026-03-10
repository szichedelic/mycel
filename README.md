# mycel

```text
              ░░░▒▒▒▓▓███▓▓▒▒▒░░░
           ░▒▓█▀▀             ▀▀█▓▒░
         ░▓█▀   ·    ·    ·      ▀█▓░
        ▒█▀  ·    ╲  │  ╱    ·     ▀█▒
       ▓█   ·   ·──╲─┼─╱──·   ·     █▓
      ▓█      ╲     ╲│╱     ╱        █▓
      █▓  · ───●─────●─────●─── ·    ▓█
      ▓█      ╱     ╱│╲     ╲        █▓
       ▓█   ·   ·──╱─┼─╲──·   ·     █▓
        ▒█▀  ·    ╱  │  ╲    ·     ▀█▒
         ░▓█▄   ·    ·    ·      ▄█▓░
           ░▒▓█▄▄             ▄▄█▓▒░
              ░░░▒▒▒▓▓███▓▓▒▒▒░░░

   ███╗   ███╗██╗   ██╗ ██████╗███████╗██╗
   ████╗ ████║╚██╗ ██╔╝██╔════╝██╔════╝██║
   ██╔████╔██║ ╚████╔╝ ██║     █████╗  ██║
   ██║╚██╔╝██║  ╚██╔╝  ██║     ██╔══╝  ██║
   ██║ ╚═╝ ██║   ██║   ╚██████╗███████╗███████╗
   ╚═╝     ╚═╝   ╚═╝    ╚═════╝╚══════╝╚══════╝
```

The network beneath your code.

`mycel` is a Rust CLI/TUI for running parallel AI coding sessions across Git worktrees. It keeps project and session state in SQLite, launches interactive backends such as `claude` or `codex`, and lets you manage sessions from a terminal dashboard instead of juggling ad hoc shells.

## What it does

- Registers Git repositories as `mycel` projects.
- Creates isolated worktrees for named sessions.
- Starts background AI sessions in `tmux`, local Docker Compose, or a remote Docker host over SSH.
- Tracks runtime metadata, session notes, and session history in SQLite.
- Banks finished sessions into portable bundles and restores them later.
- Serves the TUI over a local web server for phone or browser access.
- Lets you manage hosts, hand off runtimes, and reap idle sessions from the TUI.

## Runtime model

`mycel` currently knows about three runtime kinds:

- `tmux`: the default runtime and the most stable path.
- `compose`: a local Docker Compose-backed session runtime.
- `remote`: a Docker-over-SSH runtime for a registered remote host.

The CLI `spawn` path still creates a `tmux` session by default. The TUI spawn flow can choose `tmux`, local `compose`, or a registered remote host. Existing sessions can also be moved with `handoff`.

## Requirements

- Git with `worktree` support
- `tmux` for the default runtime
- Rust and Cargo to build `mycel`
- An AI backend CLI available on `PATH`, typically `claude` or `codex`
- Docker with Compose support for `compose` and `remote` runtimes
- SSH access to the remote machine for `remote` runtimes

## Install

`mycel` is currently a build-from-source project.

```bash
cargo build --release
./target/release/mycel --help
```

If you want it on your `PATH`:

```bash
cp ./target/release/mycel ~/.local/bin/mycel
```

On macOS you will typically want `~/bin` or `/usr/local/bin` instead.

## Quick start

Register a repo:

```bash
cd /path/to/your/repo
mycel init
```

Create a session:

```bash
mycel spawn feat-auth --backend codex --note "auth refactor"
```

Attach to it:

```bash
mycel attach feat-auth
```

Open the dashboard:

```bash
mycel
```

Useful follow-up commands:

```bash
mycel list
mycel history
mycel stop feat-auth --force
mycel kill feat-auth --remove --force
```

## Interactive setup

`mycel init` opens an interactive setup wizard unless you pass `--skip-wizard`.

The wizard currently covers:

- base branch
- worktree directory
- default backend
- common symlink paths such as `.claude` and `.env.local`
- config location, either external or in-repo

## Configuration

Global config is loaded from the first file that exists:

- `~/.mycel/config.toml`
- `~/.mycelrc`

Project config is loaded from:

- `~/.config/mycel/projects/<project>.toml` if present
- otherwise `<repo>/.mycel.toml`

Example global config:

```toml
refresh_rate = 100
backend = "codex"

[backends.codex]
command = "codex"
args = []

[backends.claude]
command = "claude"
args = []
```

Example project config:

```toml
base_branch = "main"
worktree_dir = "../.mycel-worktrees"
backend = "codex"
setup = [
  "npm install",
  "npm test"
]
symlink_paths = [
  ".claude",
  ".env.local"
]

[templates.bugfix]
setup = ["npm install"]
prompt = "Reproduce the bug, make the smallest safe fix, and run the relevant tests."
```

Notes:

- External project config is supported so you do not have to check `.mycel.toml` into the target repo.
- Worktree symlink paths are applied when a new worktree is created.
- Session names and branch names are tracked separately, so renaming a session does not require renaming the Git branch.

## Common workflows

### Parallel worktree sessions

Each session gets:

- its own Git worktree
- its own session record in the database
- its own runtime metadata
- its own note, backend, and history

This is the basic flow:

```bash
mycel spawn feat-oauth
mycel attach feat-oauth
mycel stop feat-oauth --force
mycel attach feat-oauth
mycel kill feat-oauth --remove --force
```

### Banking and restoring

Banking turns a finished session into a portable bundle, archives metadata, and optionally removes the worktree.

```bash
mycel bank feat-oauth
mycel banked
mycel unbank feat-oauth --spawn
```

You can also export and import bundles:

```bash
mycel bank-export feat-oauth -o feat-oauth.mycel-bank.tar.gz
mycel bank-import ./feat-oauth.mycel-bank.tar.gz
```

### Local Docker runtime

Use the TUI spawn flow to choose `compose`, or hand off an existing session from `tmux`:

```bash
mycel handoff feat-oauth --to compose --force
```

The compose runtime stores generated Compose files under the platform data directory and uses a per-session Compose project name such as `mycel-myproject-feat-oauth`.

### Remote hosts and handoff

Register a remote host:

```bash
mycel host add dev1 ssh://mycel@your-host --max-sessions 4
mycel host list
```

Hand off a session to that host:

```bash
mycel handoff feat-oauth --to remote --host ssh://mycel@your-host --force
```

Current remote behavior uses Docker over SSH and expects the remote machine to be reachable and ready for Docker commands. Treat this path as more environment-sensitive than the default `tmux` runtime.

### Idle runtime cleanup

Find or reap sessions that have gone idle:

```bash
mycel reap --idle-minutes 60 --dry-run
mycel reap --idle-minutes 60
```

### Web TUI

Serve the TUI over HTTP:

```bash
mycel web --host 127.0.0.1 --port 3799
```

Or bind to all interfaces and require a token:

```bash
mycel web --host 0.0.0.0 --port 3799 --token secret-token
```

TLS is supported with `--tls-cert` and `--tls-key`.

## TUI controls

Main session view:

- `a` attach
- `s` spawn
- `n` edit note
- `m` rename session
- `p` stop runtime
- `d` handoff runtime
- `v` inspect runtime details
- `b` bank session
- `u` unbank session
- `e` export banked session
- `i` import banked session
- `x` kill session
- `h` open history
- `g` open hosts
- `w` open idle runtime review
- `/` search
- `q` quit

Hosts view:

- `a` add host
- `t` enable or disable selected host
- `x` remove host
- `g` return to sessions

Idle runtime view:

- `x` reap selected runtime
- `c` reap all listed runtimes
- `+` or `-` adjust the threshold
- `w` return to sessions

## CLI reference

```text
mycel
mycel init [--skip-wizard]
mycel projects
mycel spawn <name> [--note <text>] [--template <name>] [--backend <name>] [--happy]
mycel attach <name>
mycel list
mycel history
mycel rename <from> <to>
mycel stop <name> [--force]
mycel kill <name> [--remove] [--force]
mycel bank <name> [--keep] [--force]
mycel unbank <name> [--spawn] [--force]
mycel banked
mycel bank-export <name> [-o <path>]
mycel bank-import <path> [--name <name>] [--force]
mycel handoff <name> --to <tmux|compose|remote> [--host <ssh://user@host>] [--force]
mycel host add <name> <docker-host> [--max-sessions <n>]
mycel host remove <name>
mycel host list
mycel host enable <name>
mycel host disable <name>
mycel reap [--idle-minutes <n>] [--dry-run]
mycel notify [--interval <seconds>]
mycel web [--host <addr>] [--port <n>] [--token <token>] [--tls-cert <pem>] [--tls-key <pem>]
```

## Data and storage

`mycel` stores state under the platform data directory:

- database: `<data-dir>/mycel/mycel.db`
- local compose files: `<data-dir>/mycel/compose/<project>-<session>/`
- local cache for remote compose files: `<data-dir>/mycel/remote-compose/<project>-<session>/`

On Linux, `<data-dir>` is typically `~/.local/share`. On macOS it is typically `~/Library/Application Support`.

## Development status

The core worktree, `tmux`, banking, and TUI flows are the strongest paths today. The container and remote runtime features are present and usable, but they are newer and more sensitive to host setup, Docker behavior, and backend CLI availability.

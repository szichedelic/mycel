# Ralph Agent Configuration

## Build Instructions

```bash
# Fast local build
cargo build

# Release build for realistic CLI behavior
cargo build --release
```

## Test Instructions

```bash
# Unit and integration tests
cargo test

# Linting
cargo clippy --all-targets --all-features -- -D warnings
```

## Run Instructions

```bash
# Launch the TUI
cargo run

# Common non-interactive checks
cargo run -- list
cargo run -- projects
```

## Notes
- External tools used by Mycel include `git` and `tmux`.
- Runtime work may additionally require `docker` and Docker Compose compatibility.
- Prefer verifying TUI-facing changes manually with `cargo run` when feasible.
- Keep new user-facing features accessible from the TUI, not only from CLI flags.

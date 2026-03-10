# Repository Guidelines

## Project Structure & Module Organization
`src/main.rs` defines the Clap CLI and async entrypoint. Keep command-specific behavior in `src/cli/`, where each subcommand has its own file such as `spawn.rs` or `handoff.rs`. Runtime orchestration lives in `src/session/`, SQLite persistence in `src/db/`, banking/export logic in `src/bank/`, and terminal UI code in `src/tui/`. Supporting modules such as config loading and worktree management live in `src/config/` and `src/worktree/`. Use `docs/` for design notes; `docs/plans/` and `keys/` are local-only and ignored.

## Build, Test, and Development Commands
Use Cargo for the full workflow:

- `cargo build` builds the debug binary.
- `cargo build --release` produces the optimized CLI used in the README.
- `cargo run -- --help` runs the app locally and shows top-level commands.
- `cargo test` runs the inline unit tests in `#[cfg(test)]` modules.
- `cargo fmt` applies standard Rust formatting.
- `cargo clippy --all-targets --all-features` checks for lints before review.

## Coding Style & Naming Conventions
This is a Rust 2021 codebase; follow standard 4-space indentation and keep formatting tool-driven with `cargo fmt`. Use `snake_case` for modules, functions, and test names, `PascalCase` for structs/enums, and `SCREAMING_SNAKE_CASE` for constants. Match the existing layout: new CLI commands belong in `src/cli/<command>.rs`, and runtime providers belong in `src/session/`. Prefer small, explicit helpers over dense control flow in database and runtime code.

## Testing Guidelines
Tests are currently colocated with implementation files instead of a top-level `tests/` directory. Follow the existing descriptive naming style, for example `for_kind_str_falls_back_to_tmux` and `backfill_is_idempotent`. When changing session, handoff, or database behavior, add or update unit tests in the same module and run at least `cargo test` plus a focused test command for the touched area.

## Commit & Pull Request Guidelines
Recent history uses short, scoped, imperative subjects such as `tui: add runtime selection...` and `docs(readme): add project overview...`. Keep commits single-purpose and prefer a visible scope prefix (`tui`, `session`, `docs`, `chore`). PRs should explain the behavior change, link the relevant issue, list verification commands, and include screenshots or terminal captures for TUI or web UI changes.

## Security & Configuration Tips
Do not commit local PEM files, generated artifacts, or machine-specific runtime state. Prefer external project config under `~/.config/mycel/projects/` for host-specific settings, and treat Docker host URIs, SSH endpoints, and backend credentials as sensitive.

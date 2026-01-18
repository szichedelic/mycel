# Codebase Concerns

**Analysis Date:** 2026-01-17

## Tech Debt

**Massive TUI Module:**
- Issue: `src/tui/mod.rs` is 2092 lines, containing all UI rendering, state management, and event handling in a single file
- Files: `src/tui/mod.rs`
- Impact: Difficult to navigate, modify, and maintain. High cognitive load for understanding changes.
- Fix approach: Split into separate modules: `app.rs` (state), `handlers.rs` (event handling), `render.rs` (drawing), `components/` (reusable widgets)

**Duplicated Spawn/Session Logic:**
- Issue: Session creation logic is duplicated between CLI (`src/cli/spawn.rs`) and TUI (`src/tui/mod.rs` lines 548-652)
- Files: `src/cli/spawn.rs`, `src/tui/mod.rs`
- Impact: Bug fixes must be applied twice. Behavior can diverge.
- Fix approach: Extract session creation into shared service in `src/session/mod.rs`

**Inline HTML Template:**
- Issue: Web server embeds 250+ lines of HTML/CSS/JS as a Rust string literal in `page_html()`
- Files: `src/cli/web.rs` (lines 286-544)
- Impact: Difficult to edit UI, no syntax highlighting, no hot reload during development
- Fix approach: Move to external template file or use `include_str!()` with a separate .html file

**Manual Database Migrations:**
- Issue: Schema changes handled via `ensure_sessions_*` functions that check columns individually
- Files: `src/db/mod.rs` (lines 110-184)
- Impact: Migrations pile up as separate functions. No versioning, no rollback capability.
- Fix approach: Implement proper migration system with numbered migrations and version tracking

## Known Bugs

**None explicitly marked in code.**
- The codebase has no TODO/FIXME/HACK comments, which may indicate either clean code or missing documentation of known issues.

## Security Considerations

**Web Server Token Handling:**
- Risk: Token passed as query parameter (`?token=`) appears in URLs, logs, and browser history
- Files: `src/cli/web.rs` (lines 115-127, 463-465)
- Current mitigation: Optional token-based auth
- Recommendations: Support Bearer header authentication in addition to query param. Warn users about query param risks.

**Bundle Import Without Verification:**
- Risk: `import_bundle()` extracts archives without validating content structure thoroughly
- Files: `src/bank/mod.rs` (lines 240-309)
- Current mitigation: Only reads expected files (metadata.toml, bundle.bundle)
- Recommendations: Add signature verification for bundles. Validate bundle integrity with git before restoring.

**Setup Commands Executed Unsanitized:**
- Risk: Setup commands from config are joined and passed to shell without escaping
- Files: `src/session/mod.rs` (lines 63-70)
- Current mitigation: Config is user-controlled (user's own machine)
- Recommendations: Document the security model. Consider command validation for shared configs.

## Performance Bottlenecks

**Synchronous Disk Size Calculation:**
- Problem: `dir_size_bytes()` recursively walks directories synchronously on every refresh
- Files: `src/disk/mod.rs` (lines 10-52), `src/tui/mod.rs` (line 125)
- Cause: Iterative directory traversal blocks the UI thread
- Improvement path: Move to async/background thread. Cache results. Only recalculate on session changes.

**Full Refresh on Every Action:**
- Problem: `app.refresh()` reloads all projects, sessions, history, and disk usage after every operation
- Files: `src/tui/mod.rs` (line 110-188)
- Cause: Simple implementation that reloads everything instead of targeted updates
- Improvement path: Implement incremental updates. Only refresh affected data.

**tmux capture-pane on Selection Change:**
- Problem: Preview captures 80 lines of tmux output on every selection change
- Files: `src/tui/mod.rs` (lines 1767-1789)
- Cause: External process spawned for each preview update
- Improvement path: Debounce preview updates. Consider caching recent output.

## Fragile Areas

**TUI Event Handler:**
- Files: `src/tui/mod.rs` (lines 480-1290)
- Why fragile: 800+ lines of nested match statements with terminal mode toggling
- Safe modification: Ensure raw mode is always restored. Test terminal cleanup paths.
- Test coverage: None - no automated tests exist

**Database Schema Evolution:**
- Files: `src/db/mod.rs`
- Why fragile: Manual column existence checks. Schema not versioned.
- Safe modification: Always add ensure_* functions for new columns. Never remove columns.
- Test coverage: None

**Worktree/Branch State Sync:**
- Files: `src/worktree/mod.rs`, `src/db/mod.rs`
- Why fragile: Git worktree state, branch existence, and database records can desync if operations fail midway
- Safe modification: Use transaction-like patterns. Verify state before operations.
- Test coverage: None

## Scaling Limits

**SQLite Single-File Database:**
- Current capacity: Handles typical usage well
- Limit: Concurrent write operations from multiple CLI invocations could cause locking
- Scaling path: Already appropriate for tool's scope. Add retry logic for busy database.

**In-Memory Session List:**
- Current capacity: Loads all projects/sessions into memory
- Limit: Thousands of sessions could slow UI
- Scaling path: Add pagination. Archive old history.

## Dependencies at Risk

**Axum 0.6 (Outdated):**
- Risk: Using axum 0.6 while current is 0.7+. Breaking API changes.
- Impact: Security updates may not backport. Community examples target newer versions.
- Files: `Cargo.toml` (line 21)
- Migration plan: Update to axum 0.7 - requires Router API changes

**portable-pty 0.8:**
- Risk: Niche crate for PTY handling on web server
- Impact: Limited maintenance, platform-specific bugs
- Files: `Cargo.toml` (line 24), `src/cli/web.rs`
- Migration plan: Monitor for issues. Consider alternative PTY libraries if problems arise.

## Missing Critical Features

**No Automated Tests:**
- Problem: Zero unit tests, integration tests, or end-to-end tests
- Files: None exist
- Blocks: Safe refactoring, confident releases, catching regressions
- Priority: High - should add tests for database operations, worktree management, and CLI commands

**No Error Recovery for Partial Operations:**
- Problem: Multi-step operations (bank, unbank, kill) have no rollback on failure
- Files: `src/cli/bank.rs`, `src/cli/unbank.rs`, `src/cli/kill.rs`, `src/tui/mod.rs`
- Blocks: Reliable operation in edge cases (disk full, git errors)

**No Multi-Machine Sync:**
- Problem: Sessions and config are local-only
- Blocks: Using mycel across machines
- Note: May be intentional design choice given git worktree nature

## Test Coverage Gaps

**No Tests Anywhere:**
- What's not tested: Everything - database operations, worktree creation, session management, TUI logic, CLI commands
- Files: Entire `src/` directory
- Risk: Any refactoring could introduce regressions unnoticed. Schema changes could corrupt data.
- Priority: High - start with database and worktree modules which handle persistent state

**Specific High-Risk Untested Areas:**
1. Database migrations (`src/db/mod.rs` ensure_* functions)
2. Bundle creation/restore (`src/bank/mod.rs`)
3. Worktree creation/removal (`src/worktree/mod.rs`)
4. Config parsing edge cases (`src/config/mod.rs`)

---

*Concerns audit: 2026-01-17*

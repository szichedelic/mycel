# Testing Patterns

**Analysis Date:** 2025-01-17

## Test Framework

**Runner:**
- No test framework configured
- No `#[test]` or `#[cfg(test)]` annotations found in codebase
- Standard `cargo test` would be used if tests existed

**Assertion Library:**
- Not applicable - no tests present

**Run Commands:**
```bash
cargo test              # Run all tests (no tests currently)
cargo test --no-fail-fast  # Continue on failures
cargo test -- --nocapture  # Show println output
```

## Test File Organization

**Location:**
- No test files detected
- Rust convention would be inline `#[cfg(test)]` modules or `tests/` directory

**Naming:**
- Not established - follow Rust conventions:
  - Inline: `mod tests { ... }` at bottom of source files
  - Integration: `tests/*.rs` in project root

**Recommended Structure:**
```
src/
├── db/
│   └── mod.rs         # Could add #[cfg(test)] mod tests { ... }
├── bank/
│   └── mod.rs         # Could add #[cfg(test)] mod tests { ... }
tests/
├── integration.rs     # Integration tests (if added)
└── cli_tests.rs       # CLI integration tests (if added)
```

## Test Structure

**Recommended Pattern (not yet implemented):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = "test";

        // Act
        let result = function_to_test(input);

        // Assert
        assert!(result.is_ok());
    }
}
```

**Async Test Pattern (with tokio):**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_function() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

## Mocking

**Framework:**
- Not established
- Recommended: `mockall` crate for trait-based mocking

**Current Testability Challenges:**
- `SessionManager` wraps `std::process::Command` directly
- Database uses real SQLite connections
- Git operations shell out to `git` command

**Recommended Mocking Approach:**
```rust
// Extract trait for session management
pub trait SessionOps {
    fn create(&self, ...) -> Result<String>;
    fn is_alive(&self, tmux_session: &str) -> Result<bool>;
    fn kill(&self, tmux_session: &str) -> Result<()>;
}

// In tests
#[cfg(test)]
mod tests {
    use mockall::mock;

    mock! {
        pub SessionManager {}
        impl SessionOps for SessionManager {
            fn create(&self, ...) -> Result<String>;
            fn is_alive(&self, tmux_session: &str) -> Result<bool>;
        }
    }
}
```

**What Would Need Mocking:**
- `std::process::Command` calls (git, tmux)
- File system operations
- Database connections

**What NOT to Mock:**
- Pure functions (config parsing, path manipulation)
- Data structures

## Fixtures and Factories

**Test Data:**
- Not established
- Recommended approach for database tests:

```rust
#[cfg(test)]
mod tests {
    fn test_project() -> Project {
        Project {
            id: 1,
            name: "test-project".to_string(),
            path: PathBuf::from("/tmp/test-project"),
        }
    }

    fn test_session() -> Session {
        Session {
            id: 1,
            name: "test-session".to_string(),
            branch_name: "mycel/s-1234567890".to_string(),
            worktree_path: PathBuf::from("/tmp/worktrees/test"),
            tmux_session: "mycel-test-project-s-1234567890".to_string(),
            backend: "claude".to_string(),
            note: None,
            created_at_unix: 1700000000,
        }
    }
}
```

**Location:**
- Inline in test modules or `tests/fixtures.rs` for shared fixtures

## Coverage

**Requirements:** None enforced

**View Coverage:**
```bash
# Using cargo-tarpaulin (if installed)
cargo tarpaulin --out Html

# Using cargo-llvm-cov (if installed)
cargo llvm-cov --html
```

## Test Types

**Unit Tests:**
- Not implemented
- Would test: config parsing, path manipulation, database queries
- Good candidates:
  - `src/config/mod.rs` - `resolve_backend()`, config loading
  - `src/bank/mod.rs` - `BankedItem::size_human()`
  - `src/db/mod.rs` - query methods with in-memory SQLite

**Integration Tests:**
- Not implemented
- Would test: full CLI commands with temp directories
- Challenges: requires git repos, tmux, file system

**E2E Tests:**
- Not implemented
- Would require: test fixtures with real git repos
- Tool like `assert_cmd` for CLI testing

## Common Patterns

**Recommended Async Testing:**
```rust
#[tokio::test]
async fn test_spawn_validates_project() {
    // Test that spawn fails without registered project
    let temp_dir = tempfile::tempdir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    let result = cli::spawn::run("test", None, None, None).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not registered"));
}
```

**Recommended Error Testing:**
```rust
#[test]
fn test_resolve_backend_unknown() {
    let global = GlobalConfig::default();
    let project = ProjectConfig::default();

    let result = resolve_backend(&global, &project, Some("unknown-backend"));

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Unknown backend"));
}
```

**Database Testing Pattern:**
```rust
#[test]
fn test_add_and_get_project() {
    // Use in-memory database for tests
    let conn = Connection::open_in_memory().unwrap();
    // Initialize schema...

    let db = Database { conn };
    let path = PathBuf::from("/test/path");

    db.add_project("test", &path).unwrap();
    let project = db.get_project_by_path(&path).unwrap();

    assert!(project.is_some());
    assert_eq!(project.unwrap().name, "test");
}
```

## Testable Components

**High Testability (pure logic):**
- `src/config/mod.rs` - Config parsing and backend resolution
- `src/bank/mod.rs` - `BankedItem::size_human()`, metadata serialization
- `src/confirm.rs` - Input parsing (with mock stdin)

**Medium Testability (needs mocking):**
- `src/db/mod.rs` - Use in-memory SQLite
- `src/worktree/mod.rs` - Needs temp git repos or command mocking

**Low Testability (external dependencies):**
- `src/session/mod.rs` - Shells out to tmux
- `src/tui/mod.rs` - UI rendering, terminal interaction
- `src/cli/web.rs` - HTTP server, WebSocket handling

## Recommended Test Dependencies

Add to `Cargo.toml` under `[dev-dependencies]`:
```toml
[dev-dependencies]
tempfile = "3"           # Temp directories for file tests
assert_cmd = "2"         # CLI integration testing
predicates = "3"         # Assertion helpers
mockall = "0.12"         # Mocking framework
```

## Current Test Gaps

**Critical (no tests):**
- Database schema migrations (`ensure_sessions_*` methods)
- Config parsing and defaults
- Backend resolution logic
- Banking/unbunking bundle operations

**High Priority:**
- Session lifecycle (spawn, kill, stop)
- Worktree creation and removal
- Project registration

**Medium Priority:**
- TUI rendering (consider snapshot testing with `insta`)
- CLI argument parsing

---

*Testing analysis: 2025-01-17*

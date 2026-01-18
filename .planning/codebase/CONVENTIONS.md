# Coding Conventions

**Analysis Date:** 2025-01-17

## Naming Patterns

**Files:**
- Module files use `mod.rs` inside feature directories: `src/db/mod.rs`, `src/session/mod.rs`
- CLI subcommands are individual files with snake_case: `src/cli/spawn.rs`, `src/cli/bank_export.rs`
- Single-file modules at root level: `src/confirm.rs`, `src/main.rs`

**Functions:**
- snake_case for all functions: `find_git_root()`, `create_bundle()`, `get_session_by_name()`
- Async functions use `async fn` with `run` as the main entry point in CLI modules
- Helper functions are private (no `pub`) unless needed externally

**Variables:**
- snake_case throughout: `git_root`, `session_manager`, `worktree_path`
- Descriptive names preferred over abbreviations: `bundle_path` not `bp`

**Types:**
- PascalCase for structs and enums: `Database`, `SessionManager`, `ProjectConfig`
- Struct fields use snake_case: `created_at_unix`, `worktree_path`
- Configuration structs match TOML field names via serde

**Constants:**
- SCREAMING_SNAKE_CASE: `PREVIEW_LINES`, `DOUBLE_CLICK_WINDOW_MS`, `LOGO_FRAMES`

## Code Style

**Formatting:**
- Default rustfmt (no custom config detected)
- 4-space indentation
- Line length appears to follow rustfmt defaults (~100 chars)

**Linting:**
- No clippy.toml detected - uses default clippy rules
- Edition 2021 Rust features enabled

**Braces and Blocks:**
```rust
// Functions
pub fn function_name(param: Type) -> Result<T> {
    // body
}

// Match expressions
match result {
    Ok(value) => Ok(Some(value)),
    Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
    Err(e) => Err(e.into()),
}

// If-let patterns
if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
}
```

## Import Organization

**Order:**
1. Standard library (`std::`)
2. External crates (alphabetical)
3. Internal crate modules (`crate::`)

**Pattern:**
```rust
use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;

use crate::config::ProjectConfig;
use crate::db::Database;
use crate::worktree;
```

**Path Aliases:**
- None configured - uses explicit `crate::` paths

**Grouping:**
- Multiple items from same crate grouped with braces: `use anyhow::{bail, Context, Result};`
- Nested imports for complex paths: `use crossterm::event::{self, Event, KeyCode};`

## Error Handling

**Primary Pattern:** Use `anyhow` for application errors

```rust
// Contextual errors
let git_root = worktree::find_git_root(&current_dir)?;
let project = db
    .get_project_by_path(&git_root)?
    .context("Project not registered. Run 'mycel init' first.")?;

// Early bail on validation
if from.is_empty() {
    bail!("Current session name cannot be empty");
}

// User-facing error messages
bail!("Session '{name}' already exists in this project");
```

**Result Types:**
- Functions return `Result<T>` (using `anyhow::Result`)
- Option used for nullable database lookups: `Result<Option<Session>>`

**Error Propagation:**
- Use `?` operator consistently
- Add context with `.context("message")` for user-facing errors
- Specific error types converted with `.into()`

## Logging

**Framework:** `tracing` with `tracing-subscriber`

**Initialization:**
```rust
// In main.rs
tracing_subscriber::fmt::init();
```

**Patterns:**
- Console output preferred over tracing for user messages
- `println!()` for success messages and progress
- `eprintln!()` for warnings
- `tracing` available but minimally used in current codebase

## Comments

**When to Comment:**
- Doc comments (`///`) for public struct/enum definitions
- Inline comments for non-obvious logic
- ASCII art preserved with raw strings

**JSDoc/TSDoc:**
- Not applicable (Rust codebase)
- Rust doc comments (`///`) used sparingly

**Examples:**
```rust
/// Find the git repository root from a given path
pub fn find_git_root(from: &Path) -> Result<PathBuf> {

/// Create a new git worktree. Returns (worktree_path, session_id)
/// The worktree starts on a temp branch that can be renamed later.
pub fn create(git_root: &Path, config: &ProjectConfig) -> Result<(PathBuf, String)> {
```

## Function Design

**Size:**
- Most functions under 50 lines
- TUI module (`src/tui/mod.rs`) is largest at ~2000 lines - handles all UI rendering

**Parameters:**
- Use references (`&str`, `&Path`) over owned types when not consuming
- Group related parameters in structs for complex operations: `NewSession`
- Optional parameters use `Option<&str>` pattern

**Return Values:**
- `Result<T>` for fallible operations
- Tuples for multiple returns: `Result<(PathBuf, String)>`
- Unit `()` for side-effect-only functions wrapped in Result

## Module Design

**Exports:**
- Each feature directory has `mod.rs` as public interface
- Re-export key types at module level
- CLI module uses barrel file pattern in `src/cli/mod.rs`

**Barrel Files:**
```rust
// src/cli/mod.rs
pub mod attach;
pub mod bank;
pub mod bank_export;
// ... etc
```

**Visibility:**
- `pub` only for items needed outside module
- Default private for internal helpers

## Struct Patterns

**Configuration:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default = "default_base_branch")]
    pub base_branch: String,
    #[serde(default)]
    pub setup: Vec<String>,
}
```

**Data Models:**
```rust
#[derive(Debug, Clone)]
pub struct Session {
    pub id: i64,
    pub name: String,
    pub worktree_path: PathBuf,
    // ...
}
```

**Builder/Input Structs:**
```rust
pub struct NewSession<'a> {
    pub project_id: i64,
    pub name: &'a str,
    // uses references to avoid allocation
}
```

## Platform-Specific Code

**Conditional Compilation:**
```rust
#[cfg(unix)]
pub fn filesystem_usage(path: &Path) -> Option<DiskUsage> {
    // Unix implementation
}

#[cfg(not(unix))]
pub fn filesystem_usage(_path: &Path) -> Option<DiskUsage> {
    None
}
```

**OS-Specific Imports:**
```rust
#[cfg(target_os = "macos")]
use anyhow::Context;
#[cfg(target_os = "macos")]
use std::process::Command;
```

## Async Patterns

**Runtime:** Tokio with full features

**CLI Entry Points:**
```rust
pub async fn run(name: &str) -> Result<()> {
    // CLI commands are async but mostly do sync work
}
```

**Main:**
```rust
#[tokio::main]
async fn main() -> Result<()> {
    // ...
}
```

## String Handling

**Interpolation:**
```rust
// Format strings with variable capture
let tmux_session_name = format!("mycel-{project_name}-{session_name}");
bail!("Session '{name}' already exists");
```

**Path Conversion:**
```rust
// Path to string for external commands
&worktree_path.to_string_lossy()
```

**Escaping:**
```rust
fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}
```

---

*Convention analysis: 2025-01-17*

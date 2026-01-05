---
name: work-on-issue
description: Pick a GitHub issue, create a branch, design and implement the solution with organized commits
---

# Work on Issue

Automated workflow for picking up and implementing GitHub issues.

## Workflow

### 1. List and Select Issue

```bash
gh issue list --state open --limit 20
```

Present issues to user and ask which one to work on. If user doesn't specify, recommend the easiest/quickest win.

### 2. Create Branch

Create a branch named after the issue:
- Format: `issue-{number}-{short-description}`
- Example: `issue-7-stop-vs-kill`

```bash
git checkout -b issue-{number}-{short-description}
```

### 3. Design Phase

**Use ultrathink mode** to thoroughly analyze:
- Read the issue details: `gh issue view {number}`
- Explore relevant code using Glob, Grep, Read
- Consider architectural implications
- Identify all files that need changes

**Use superpowers:brainstorming** if the solution has multiple valid approaches.

**Use superpowers:writing-plans** to create a detailed implementation plan.

### 4. Implementation Phase

**Use superpowers:test-driven-development** if adding testable functionality.

**Prefer TUI updates over CLI options** - all user-facing features should be accessible from the TUI.

Implement incrementally, committing as you go.

### 5. Commit Organization

Use the commit-organizer agent or manually organize commits with:

**Commit message format:**
```
{scope}: {single line description}
```

**Scopes:**
- `tui` - TUI changes
- `db` - Database changes
- `session` - Session management
- `bank` - Banking functionality
- `worktree` - Git worktree operations
- `config` - Configuration changes
- `fix` - Bug fixes
- `refactor` - Code refactoring

**Rules:**
- Single line commit messages only
- No co-authorship tags
- Granular, atomic commits (one logical change per commit)
- Use imperative mood ("add" not "added")

**Examples:**
```
tui: add session duration display
tui: add confirmation prompt before kill
db: add created_at column to sessions table
session: track session start time
```

### 6. Verification

**Use superpowers:verification-before-completion** to:
- Run `cargo build --release`
- Run `cargo clippy` if available
- Test the feature manually
- Verify all changes work as expected

### 7. Close Issue Reference

After implementation, note in final commit or PR:
```
Closes #{issue_number}
```

## Quick Reference

| Step | Action |
|------|--------|
| List issues | `gh issue list` |
| View issue | `gh issue view {n}` |
| Create branch | `git checkout -b issue-{n}-{desc}` |
| Build | `cargo build --release` |
| Test TUI | Run `mycel` and verify |

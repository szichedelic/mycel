---
name: work-on-issue
description: Pick a GitHub issue, create a branch, plan and implement the solution, and keep commits organized for this repo. Use when asked to work on a GitHub issue, start an issue branch, or follow the /work-on-issue workflow.
---

# Work On Issue

## Overview

Implement a GitHub issue end-to-end: select the issue, create a branch, plan, implement, organize commits, and verify before completion.

## Workflow

### 1. List and select issue

List open issues and ask the user to choose.

```bash
gh issue list --state open --limit 20
```

If the user has no preference, recommend the quickest win based on scope and complexity.

### 2. Review issue details

Read the issue and summarize requirements, acceptance criteria, and any constraints.

```bash
gh issue view {number}
```

### 3. Create branch

Use the format `issue-{number}-{short-description}` in kebab case.

```bash
git checkout -b issue-{number}-{short-description}
```

### 4. Plan and design

Scan relevant code, identify all files to change, and call out edge cases.

When the work is multi-step or risky, write a short plan before coding.

If multiple approaches are plausible, compare tradeoffs and confirm the approach with the user.

### 5. Implement

Prefer TUI updates over CLI-only options for user-facing features.

Implement incrementally and commit as you go.

### 6. Organize commits

Use this commit message format:

```
{scope}: {single line description}
```

Scopes: `tui`, `db`, `session`, `bank`, `worktree`, `config`, `fix`, `refactor`

Rules:
- Use a single line only.
- Use imperative mood ("add" not "added").
- Use one logical change per commit.
- Do not add co-authorship tags.

### 7. Verify

Run required checks and validate behavior.

```bash
cargo build --release
cargo clippy
```

Manually verify the feature in the TUI by running `mycel`.

### 8. Finish

Include `Closes #{issue_number}` in the final commit message or PR body.

Push the branch or open a PR only when the user asks.

## Quick reference

| Step | Command |
| --- | --- |
| List issues | `gh issue list --state open --limit 20` |
| View issue | `gh issue view {number}` |
| Create branch | `git checkout -b issue-{number}-{short-description}` |
| Build | `cargo build --release` |

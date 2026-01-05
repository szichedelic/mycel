---
description: Pick a GitHub issue, create branch, design and implement with organized commits
---

# Work on Issue

## Step 1: List Issues

Run `gh issue list --state open` and present the issues to the user.

Ask: "Which issue would you like to work on?"

If no preference given, recommend the quickest win based on complexity.

## Step 2: Analyze Issue

Once an issue is selected:
```bash
gh issue view {number}
```

Read the full issue description and requirements.

## Step 3: Create Branch

```bash
git checkout -b issue-{number}-{short-kebab-description}
```

## Step 4: Design (Use Ultrathink)

Think deeply about the implementation:
- Explore relevant code with Glob, Grep, Read
- Identify all files needing changes
- Consider edge cases and architectural implications
- If multiple approaches exist, use superpowers:brainstorming

Create a mental or written plan before coding.

## Step 5: Implement

- **TUI-first**: Always prefer adding features to the TUI over CLI-only options
- Implement incrementally
- Build frequently: `cargo build --release`
- Test as you go

## Step 6: Organize Commits

Create granular, atomic commits with this format:
```
{scope}: {single line description}
```

**Scopes:** tui, db, session, bank, worktree, config, fix, refactor

**Rules:**
- Single line only
- NO co-authorship tags
- Imperative mood ("add" not "added")
- One logical change per commit

## Step 7: Verify

Before claiming done:
- Run `cargo build --release` - must succeed
- Run the TUI and manually verify the feature works
- Check for warnings with `cargo clippy` if available

## Step 8: Complete

Push the branch and optionally create a PR:
```bash
git push -u origin {branch-name}
gh pr create --title "..." --body "Closes #{issue_number}"
```

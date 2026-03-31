---
name: commit-message
description: Generate a commit message from uncommitted changes. Use when the user wants to create a commit message, says "commit message", "generate commit", "write commit", or wants to know what to commit. Invoke with /commit-message.
---

# Commit Message Generator

Generates a commit message based on all staged changes in the working directory.

## Format

```
Type(scope): Short description in a sentence
```

## Rules

1. **Type**: Use one of:
   - `Feat` — new feature or functionality
   - `Fix` — bug fix
   - `Refactor` — code restructuring without behavior change
   - `Chore` — maintenance, deps, config, docs
   - `Style` — visual/UI changes without logic change
   - `Test` — adding or updating tests only
   - `Perf` — performance improvement

2. **Scope**: The affected component or crate in parentheses (e.g., `desktop`, `top-header`, `sidebar`, `core`, `api-client`). Derive from the changed file paths.

3. **Description**: One concise sentence describing what changed. Start with a capital letter, no period at the end.

## Steps

1. Run `git diff --cached --stat` to see which files are staged.
2. Run `git diff --cached` to understand the actual changes.
3. Determine the type, scope, and write a short description.
4. Output ONLY the ready-to-paste command:

```
git commit -m "Type(scope): Description here"
```

5. Do NOT run the commit. Do NOT add extra explanation. Just the command block.

## Examples

- `git commit -m "Style(top-header): Restyle header with grouped nav icons and indigo accent"`
- `git commit -m "Feat(sidebar): Add mini sidebar with vertical icon navigation"`
- `git commit -m "Fix(desktop): Remove panel separator line and unused constants"`
- `git commit -m "Chore(deps): Update egui to 0.35"`

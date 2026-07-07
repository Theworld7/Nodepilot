# ADR 0006: Git branch switching in project rows

## Status
Accepted

## Context
Users bind projects to Node versions and run dev servers from the panel. In multi-branch workflows, switching branches before starting a server is a frequent operation. Currently, the user must switch to a terminal, run `git checkout`, and return to the panel — breaking the flow.

This ADR covers adding git branch display and checkout capability to `ProjectRow` components.

## Decision

### Rust backend: two commands

Two new `#[tauri::command]` handlers in `commands.rs`, both using `tokio::process::Command` to shell out to git (consistent with the existing `start_dev_server` and `detect_pm` pattern):

1. **`list_git_branches(path)`** — runs `git branch` in the project directory, parses the output, returns:
   ```rust
   struct GitBranches {
       branches: Vec<GitBranch>,
   }
   struct GitBranch {
       name: String,
       is_current: bool,
   }
   ```

2. **`checkout_branch(path, branch)`** — runs `git checkout <branch>` in the project directory. Returns `Ok(())` on success or `AppError` with the git stderr message on failure.

Two separate commands (not a combined checkout+list) for single responsibility.

### Frontend: inline branch display + popup switcher

- **Layout**: branch info displayed below the project path as `🌿 <branch-name>`, using a `t-popup` to show the full branch list on click.
- **Loading**: `list_git_branches` is called on component mount for every project. Branch data is small (< 20 branches), so one-time loading is simpler than lazy loading.
- **Switching**: clicking a non-current branch in the popup calls `checkout_branch`. On success, the branch name updates in place (no toast). On failure, `MessagePlugin.error` displays the git error.
- **Non-git projects**: if `list_git_branches` fails (no git repo, git not installed), the branch line is silently absent — no placeholder or error message.
- **Running server guard**: if the project's dev server is running (`running === true`), the popup shows a warning message instead of the branch list: "请先停止 Dev Server". The guard is frontend-only; the Rust command has no server awareness.

### No stash / fetch / remote branches

- Only local branches are displayed. `git fetch` is not called.
- Dirty working directory is not auto-stashed. `git checkout` will fail and the stderr message is shown to the user.
- The feature is scoped strictly to `git checkout` for switching context before running project commands.

## Consequences

Positive:
- Keeps the user in the panel for the branch→dev-server workflow
- Minimal backend surface — two simple commands shelling out to git
- Non-git projects are unaffected (no extra UI noise)
- Consistent with existing project patterns (IPC commands, inline project info)

Negative:
- Shelling out to git ties us to the system git installation
- No safeguard against detached HEAD or other unusual git states
- Branch list can become stale if the user switches branches externally (from a terminal)

## Alternatives Considered
- **Use libgit2 (git2 crate) instead of shelling out** — rejected for initial implementation. Adds a C dependency and compilation complexity. Shelling out is simpler and matches the project's existing approach.
- **Lazy-load branches on click** — rejected in favor of mount-time loading. Branch data is small enough that one call is simpler than splitting current-branch detection and full-list loading into two commands.
- **Display branch in the settings drawer** — rejected. Branch switching is a frequent action; burying it in a drawer adds friction.
- **Combined checkout+list command** — rejected for single-responsibility. Two commands give the frontend more control over the refresh flow.
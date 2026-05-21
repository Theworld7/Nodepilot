# ADR 0001: Rust backend owns all version management logic

## Status
Accepted

## Context
The app needs to list, download, install, switch, and delete Node.js versions. Two approaches were considered: (a) implement the logic from scratch in Rust, or (b) shell out to an existing CLI tool (nvm, fnm) as a sidecar process.

Shelling out would have been faster to implement initially, but introduces coupling to external CLI behavior, parsing fragility, and dependency on those tools' installation on the user's machine.

## Decision
Rust backend (Tauri) will implement all version management logic directly:
- Fetch version list from remote registry (nodejs.org or mirror)
- Download binary archives (tar.gz / zip)
- Extract and place under `~/.nodepilot/versions/`
- Manage the `current` symlink
- Delete installed versions

Communication with the frontend uses Tauri IPC (invoke + events).

## Consequences
Positive:
- No external runtime dependencies (no nvm/fnm required on user's machine)
- Full control over error handling, progress reporting, and edge cases
- Consistent behaviour across macOS and Windows

Negative:
- Higher initial implementation cost
- Must handle extraction, symlink creation, and platform differences ourselves
- Need to keep up with Node.js release format changes

## Alternatives Considered
- Sidecar approach (bundle fnm binary, call it from Rust) — rejected to avoid dependency coupling
- Shell out to nvm (requires nvm installed) — rejected for inconsistent user setups

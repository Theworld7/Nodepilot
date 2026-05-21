# ADR 0003: Dynamic tray icon with major version number

## Status
Accepted

## Context
The tray icon is the user's primary glance point. Two approaches: a static Node.js logo that never changes, or an icon that dynamically shows the active major Node version number (e.g. "24").

A static icon is simpler but forces the user to open the panel just to check which version is active. A version-number icon gives the user instant awareness without interaction.

## Decision
The tray icon will be generated at runtime, compositing the active Node.js major version number (as text) onto a base icon. The Rust backend will:
- Maintain a base icon template (Node.js green logo)
- Overlay the version number text onto it
- Update the icon whenever the active version changes (symlink update, app startup)

## Consequences
Positive:
- User can see the active version at a glance without opening the panel
- The icon acts as a persistent version indicator

Negative:
- Requires runtime icon generation (platform-specific: .png on macOS/.ico on Windows)
- Icon rendering quality depends on font/DPI handling
- Added complexity over a static icon

## Alternatives Considered
- Static Node.js logo icon — rejected; loses the "at a glance" value
- Tooltip text only (native tray tooltip) — rejected; tooltip requires hover, not glanceable

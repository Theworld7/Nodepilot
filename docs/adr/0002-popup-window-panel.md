# ADR 0002: Popup window for version panel (not native menu)

## Status
Accepted

## Context
When the user clicks the tray icon, we need to display the version list and allow interaction (search, install, switch, delete). Two approaches: a native OS menu via TrayMenu, or a Tauri child window (popup).

A native menu is lightweight and feels platform-native, but cannot host rich interactive components (search input, progress bars, styled buttons, scrolling lists with sub-info). It also has severe size limits on some platforms.

## Decision
The panel will be implemented as a phone-sized (~375×667) Tauri child window with Vue 3 + tdesign-mobile-vue components. It is opened/hidden on tray icon click and positioned near the tray icon.

## Consequences
Positive:
- Full HTML/CSS UI — search, progress bars, rich list items, animations
- Consistent UI across macOS and Windows
- Reuses the existing Vue 3 / tdesign-mobile-vue stack
- No platform-imposed menu size limits

Negative:
- Window positioning logic varies per platform (must detect tray icon location)
- Heavier than a native menu (window creation cost)
- Window focus/close management is more complex

## Alternatives Considered
- Native tray menu (TrayMenu API) — rejected for insufficient interactivity
- A full desktop window (not phone-sized) — rejected; tdesign-mobile fits the phone form factor better

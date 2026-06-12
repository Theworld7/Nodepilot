# ADR 0004: Regular desktop window (not popup panel)
Status: Accepted
Supersedes: ADR 0002

Context:
The original design implemented the version panel as a phone-sized popup
window that auto-hides on focus loss — mimicking a dropdown tray menu.
This was chosen because the app lived entirely in the tray.

After user feedback, the app now needs a regular desktop window:
- Visible on launch like a normal application
- Has native title bar with close/minimize buttons
- Does not auto-hide on focus loss
- Close button hides to tray instead of quitting

Decision:
The window is a traditional desktop window — fixed 375×667, decorated,
non-transparent, always visible when open. Tray icon click shows and
focuses the window (no toggle/hide). Closing via title bar X button
hides the window to tray; the app persists via the tray icon.

Consequences:
Positive:
- Familiar desktop application behavior
- User can keep the window open while working
- No special focus management logic

Negative:
- Larger visual footprint than popup
- User must use tray icon to re-show after closing

Changes from ADR 0002:
- `decorations: true` (was false)
- `transparent: false` (was true)
- No auto-hide on focus loss (was present)
- Tray click: always show+focus (was toggle visibility)
- Close button: prevent close, hide to tray (was close/exit)
- Startup: window visible (was hidden in release builds)

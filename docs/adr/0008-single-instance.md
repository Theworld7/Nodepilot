# ADR 0008: Single instance enforcement

## Status
Accepted

## Context
The app lives in the tray: clicking the window's close button hides it (`CloseRequested` → `prevent_close` + `hide()` in `lib.rs`) while the process keeps running dev servers. This makes "launch it again" a normal user action — for example double-clicking the desktop shortcut when the window is hidden, or re-running the exe.

Before this ADR, each launch started an independent process with its own `AppState`, its own tray icon, and its own `HashMap` of dev server handles. Two visible symptoms:

- Duplicate tray icons that are impossible to tell apart.
- The second process never sees the running dev servers of the first, so `stop_dev_server` on a "stale" row fails while the real server keeps holding the port. `cleanup_all_servers` only cleans the process that owns the state, so the second instance's exit leaves the first instance's children untouched — and vice versa.

The tray icon is the app's primary entry point, so "second launch" must mean "show the existing window", not "start a second app".

## Decision
Register `tauri-plugin-single-instance` as the **first** plugin in the builder chain in `lib.rs` (the plugin requires this; it installs its IPC listener before anything else):

```rust
.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}))
```

The callback runs in the **first** (already running) instance. It restores and focuses the window, then the second process exits on its own.

`unminimize()` is part of the sequence on purpose: the close-to-tray path leaves the window hidden, and `show()` alone does not always restore foreground focus from a hidden/minimized state on Windows.

Argument forwarding (`argv`, `cwd`) is ignored — the app takes no CLI arguments today. Deep-link-style handling (opening a project path passed on the command line) is out of scope.

## Consequences
Positive:
- One process, one `AppState`, one tray icon — dev server ownership stays unambiguous
- Re-launching from the shortcut/tray behaves like a normal desktop app
- No frontend change: the plugin is backend-only and exposes no commands, so `capabilities/default.json` needs no new permission

Negative:
- `pnpm tauri dev` no longer supports two concurrent dev instances. In practice the second one already fails earlier at Vite's `strictPort: 5199`, so nothing real is lost — but it does mean single-instance must be tested against a release build
- The mutex/pipe is keyed on the Tauri identifier (`com.nodepilot`), so a debug build and a release build cannot run at the same time either

## Alternatives Considered
- **Named mutex + manual `SetForegroundWindow` via Win32** — rejected: duplicates what the plugin already does portably, and requires `cfg(windows)` code plus a macOS fallback.
- **Kill the second process silently without focusing** — rejected: user-visible no-op (clicking the icon appears to do nothing while the window stays hidden).
- **Allow multiple instances but share `AppState` via IPC** — rejected as disproportionate: the tray + dev-server model assumes a single owner; sharing server handles across processes means serializing process ownership for no user benefit.
- **macOS-only `LSMultipleInstancesProhibited` / `open -n` handling** — rejected: macOS already enforces single instance at the LaunchServices level when launched from Finder; the plugin additionally covers command-line launches with one code path for both platforms.

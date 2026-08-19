# ADR 0007: App update detection via GitHub Releases API

## Status
Accepted

## Context
The app should notify users when a new version is published. `tauri-plugin-updater` is already a dependency and registered in release builds (`lib.rs`), with a pubkey and endpoint (`releases/latest/download/latest.json`) configured in `tauri.conf.json`. However, the release process only uploads the NSIS + MSI installers — never `latest.json` — so the plugin's `check()` would hit a 404. The updater is effectively inert configuration, and a future reader might "fix" that by wiring the plugin up without realizing the release pipeline cannot support it yet.

Enabling the plugin properly would require publishing a signed `latest.json` with every release (minisign private key + a pipeline change to the manual API-based release process) — disproportionate for a detection-only requirement.

## Decision

Implement lightweight update **detection** (not auto-update):

- A new `check_app_update` command queries the GitHub Releases API (`api.github.com/repos/Theworld7/Nodepilot/releases/latest`) using the existing HTTP client, compares the tag (`v0.2.8`) against the app version, and returns `{ version, url }` when newer.
- The panel shows a tdesign confirm dialog ("发现新版本 vX.Y.Z，是否前往下载？"); confirming opens the release page in the system browser via the opener plugin. Installation stays manual.
- Backend-owned, consistent with the architecture (backend owns all version management logic).
- Checked once on launch. Failures — network error, timeout (5s), non-standard tag, JSON parse — are silent (debug log only). Debug builds skip the check entirely.
- No skip-this-version persistence.

## Consequences

Positive:
- Zero change to the manual release process (NSIS + MSI upload stays as-is)
- No dependency on the minisign private key
- No startup blocking — check runs async after the window loads

Negative:
- Update flow is detect + browser, not in-app download/install
- GitHub API unreachable (e.g. slow networks) silently disables the feature — accepted, no manual retry entry

## Alternatives Considered
- **tauri-plugin-updater full flow** — rejected: requires signed `latest.json` per release; the manual API-based release process (no `gh` CLI) makes signing disproportionate for now. The registered-but-inert plugin stays for a future full auto-update implementation; revisit when `latest.json` publishing is feasible.
- **Frontend fetch to api.github.com** — rejected: backend owns network logic; avoids webview/CORS concerns.
- **Manual check entry + skip-version** — rejected in design review: launch-only check with no recovery path if the user ever skipped; keeping the surface minimal.
- **Native confirm dialog** — rejected in design review in favor of tdesign's `DialogPlugin.confirm`, matching the app's component library.

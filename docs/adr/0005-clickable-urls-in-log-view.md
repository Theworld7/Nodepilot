# ADR 0005: Clickable URLs in dev-server log view open in system browser

Status: Accepted

Context:
The dev-server log view (LogView.vue) renders captured stdout/stderr as
ANSI-colored HTML. URLs (e.g. Vite's `Local: http://localhost:3006/`) appear
as plain text and cannot be opened. The log window is a separate WebviewWindow
with no navigation guard, so a live `<a href>` would navigate the log window
away from the stream. Opening a URL also requires a system-level "open"
primitive, which Tauri does not expose without a plugin.

Decision:
URLs matching `http://` / `https://` are linkified in the log view and opened
in the system default browser on single click, via the `tauri-plugin-opener`
plugin (`openUrl`). The log window itself never navigates: link clicks are
intercepted (preventDefault) and routed through the opener plugin.
Linkification runs on the raw ANSI text (not the colored HTML), tolerating ANSI
escape sequences embedded inside the URL so that Vite's colored/bold `Local:`
URL becomes a single link; the embedded codes are stripped to derive the href
while the on-screen text keeps its color. Links remain clickable in selection
mode; selection stays driven solely by line-number/checkbox clicks.

Links are plain `<a>` anchors injected into the per-line HTML (no real `href`,
so the webview can never navigate; the target rides in `data-href`). Because
scoped styles do not reach `v-html` content, the link styling is applied via
`:deep()` from the log view's scoped stylesheet.

Consequences:
Positive:
- Dev-server URLs (the primary case) open in the user's real browser
- Log stream stays intact — the webview never navigates
- Official, cross-platform plugin handles Windows/macOS/Linux differences

Negative:
- New plugin dependency touching 5 files (Cargo.toml, lib.rs, capabilities,
  package.json, LogView.vue)
- Only http/https URLs are recognized; file://, bare host:port, and source
  locations (src/Foo.vue:12) are intentionally out of scope for now

Considered Options:
- tauri-plugin-shell `open()` — rejected: heavier, broader permission surface
  than needed for a single "open URL" primitive
- Hand-written Rust command (`start`/`open`/`xdg-open`) — rejected: per-platform
  process/unsafe code to maintain for no real benefit
- Navigate the log webview to the URL — rejected: destroys the running log view

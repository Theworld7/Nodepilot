# PRD: nodepilot — Node.js Version Manager GUI

## Problem Statement

Developers who use Node.js daily need to manage multiple installed versions: install new releases, switch between projects that require different Node versions, and clean up old versions. Existing solutions are CLI-only (nvm, fnm, n) with no GUI, making them less discoverable for visual-oriented users and inconvenient for quick operations like checking the active version or installing a new LTS release.

## Solution

A system-tray desktop app (nodepilot) built with Tauri + Vue 3 that provides a graphical interface for managing Node.js versions. The tray icon shows the active Node.js major version at a glance. Clicking opens a phone-sized panel with a searchable, filterable version list displaying install status, release date, and LTS labels. Users can install, switch, and delete versions entirely from the GUI. Version management operations are fast because the Rust backend handles everything directly without shelling out to external tools.

## User Stories

1. As a developer, I want the app to start automatically when I log in, so that the tray icon is always available without manual launch.

2. As a developer, I want to see my active Node.js version in the system tray icon, so that I can tell which version is current without opening any window.

3. As a developer, I want to click the tray icon to open a version list panel, so that I can see all available Node.js versions in one place.

4. As a developer, I want the panel to show cached version data immediately and refresh in the background, so that I don't wait for a network request on every open.

5. As a developer, I want the version list to show the version number, install status, release date, and LTS label for each entry, so that I can make informed decisions about which version to install or use.

6. As a developer, I want to filter the version list by major version number (e.g. typing "24" shows all 24.x.x releases), so that I can quickly find versions in a specific major line.

7. As a developer, I want the panel to default to showing only LTS versions with an option to show all versions, so that I am guided toward stable releases.

8. As a developer, I want to install a Node.js version by clicking an install button, so that I can add new versions without using a terminal.

9. As a developer, I want to see download and extraction progress during installation, so that I know the operation is proceeding and how long it will take.

10. As a developer, I want to switch the active Node.js version by clicking an activate button, so that my terminal `node` command uses a different version.

11. As a developer, I want to delete an installed Node.js version by clicking a delete button, so that I can reclaim disk space from unused versions.

12. As a developer, I want a confirmation dialog before deleting a version, so that I don't accidentally remove a version I need.

13. As a developer, I want to be prompted to migrate my global npm packages when I switch to a new version, so that tools like pnpm, yarn, or tsx remain available after switching.

14. As a developer using Windows, I want the app to handle symlink creation (with admin privileges), so that version switching works correctly on Windows.

15. As a developer behind a firewall, I want to configure a custom mirror URL for the Node.js binary registry, so that I can download versions through a local or regional mirror.

16. As a developer who is offline, I want to see my cached version list and installed versions, so that I can still switch between installed versions without network access.

17. As a developer, I want a setup guide shown on first launch that tells me how to add `~/.nodepilot/current/bin` to my PATH, so that I can use activated versions from the terminal.

18. As a developer, I want the app to update itself automatically when a new version of nodepilot is released, so that I always have the latest features and fixes.

## Implementation Decisions

### Modules

#### Rust Backend — Domain Layer

**`VersionFetcher`**
- Fetches `index.json` from `https://nodejs.org/dist/index.json` (or configured mirror)
- Parses and caches the version list to `~/.nodepilot/cache/versions.json`
- Returns a sorted list of `VersionInfo` structs containing: version, date, lts flag, files
- On panel open: returns cached data immediately, then refreshes in background
- Handles network errors gracefully — returns cached data with a stale flag

**`VersionInstaller`**
- Downloads the platform-appropriate binary archive (darwin-x64, darwin-arm64, win-x64, etc.)
- Extracts archive to `~/.nodepilot/versions/<version>/`
- Emits progress events via Tauri event system during download and extraction
- Deletes the archive after successful extraction
- Validates checksum if available

**`VersionActivator`**
- Updates `~/.nodepilot/current` symlink to `~/.nodepilot/versions/<version>/`
- On Windows, requires admin privileges (app runs as admin)
- Before changing, reads the previously active version's global npm packages via `npm ls -g --depth=0 --json`
- Emits a migration offer event to the frontend

**`VersionDeleter`**
- Removes the version directory from `~/.nodepilot/versions/<version>/`
- Refuses to delete the currently active version
- Emits success/failure event

**`NpmMigrator`**
- Reads global packages list from the previously active version (provided by VersionActivator)
- Runs `npm install -g <packages>` using the newly activated version's npm
- Reports results (success/failure per package) via event

**`TrayIcon`**
- Maintains a base template icon
- On version change or app start: composites the active major version number onto the base icon
- Updates the tray icon handle with the new image
- Platform differences: PNG for macOS, ICO for Windows (generated via image crate)

**`WindowManager`**
- Creates/positions the panel window on tray icon click
- Toggles visibility (click to show, click again to hide, or blur to hide)
- On macOS: positions below the status bar icon
- On Windows: positions above the system tray area
- Window size: ~375×667, centered relative to tray icon when possible

**`AppUpdater`**
- Uses Tauri updater plugin configured with GitHub Releases
- Checks for updates on startup
- Downloads and applies updates in the background, prompts restart

#### Vue 3 Frontend — Presentation Layer

**`VersionListPanel`** — Main panel component
- Receives version list from Rust via Tauri IPC
- Displays flat list with search/filter input
- Each row shows: version number, LTS badge, release date, install status badge, action button (install/activate/delete/in-use)
- Search filters by major version prefix match
- LTS toggle (show LTS only vs all)
- Pull-to-refresh triggers background cache refresh

**`ProgressIndicator`** — Inline progress bar
- Listens to Tauri events from installer
- Shows progress percentage and current step (downloading / extracting)

**`SetupGuide`** — First-launch overlay
- Shows only on first run (flag in app data)
- Displays the PATH setup command: `export PATH="$HOME/.nodepilot/current/bin:$PATH"`
- Detect shell preference and show `.zshrc` or `.bashrc` instructions accordingly
- Dismissible after user confirms setup

**`MigrationPrompt`** — Dialog shown on version switch
- Lists the global packages detected from the old version
- Offers "Migrate" / "Skip" buttons
- Shows migration progress if accepted

### IPC Contract

| Direction | Command / Event | Payload |
|-----------|---------------|---------|
| Frontend → Backend | `get_versions` | — |
| Frontend → Backend | `install_version { version: String }` | — |
| Frontend → Backend | `activate_version { version: String, migrate: bool }` | — |
| Frontend → Backend | `delete_version { version: String }` | — |
| Frontend → Backend | `get_config` | — |
| Frontend → Backend | `set_config { mirror_url: String?, auto_start: bool? }` | — |
| Backend → Frontend | `install_progress { version: String, stage: String, percent: f64 }` | Event |
| Backend → Frontend | `migration_offer { from_version: String, packages: Vec<String> }` | Event |
| Backend → Frontend | `migration_progress { package: String, status: String }` | Event |
| Backend → Frontend | `versions_updated` | Event |

### Directory Layout

```
~/.nodepilot/
├── current -> versions/v24.1.2/          (symlink)
├── versions/
│   ├── v18.20.0/
│   │   ├── bin/node
│   │   ├── bin/npm
│   │   └── ...
│   └── v24.1.2/
│       ├── bin/node
│       ├── bin/npm
│       └── ...
└── cache/
    └── versions.json
```

## Testing Decisions

### What Makes a Good Test
- Tests assert on external behaviour: given input X, the module produces output Y or side-effect Z
- Tests do not assert on internal calls, intermediate state, or implementation structure
- Network and filesystem are mocked at the boundary (HTTP client trait, temp directories)
- Tests cover: happy path, error cases, edge cases, offline scenarios

### Modules to Test

**`VersionFetcher`**
- Returns parsed version list from a mock HTTP response
- Falls back to cache when HTTP fails
- Returns error for malformed remote JSON
- Filters correctly by platform (darwin-arm64, darwin-x64, win-x64)

**`VersionInstaller`**
- Downloads, extracts, and places files in the correct directory
- Emits progress events with correct percentages
- Deletes archive after extraction
- Returns error for download failure, extraction failure, disk full
- Handles pre-existing version directory (skip or overwrite)

**`VersionActivator`**
- Creates correct symlink from current to version directory
- Reads old version's npm global packages before switching
- Returns error if target version directory doesn't exist
- Handles broken current symlink gracefully

**`VersionDeleter`**
- Removes version directory from disk
- Returns error when trying to delete the active version
- Returns error when version directory doesn't exist

**`NpmMigrator`**
- Runs `npm install -g` with correct package list
- Reports success/failure per package
- Handles npm not being available

### Prior Art
No existing tests in the codebase yet. Test files should be placed alongside source files (`src/<module>/tests/`) or in `tests/` for integration tests. Use Rust's built-in `#[cfg(test)]` and `#[test]` conventions.

## Out of Scope

- Installing Node.js versions from source code (build from source) — only prebuilt binaries
- Managing npm/yarn/pnpm versions separately from their bundled Node.js
- Running Node.js versions in isolated shells or temporary environments
- A full, resizable desktop window — the panel is fixed at phone size
- Docker-based Node.js version management
- Integration with editor/IDE tooling
- Package manager proxy configuration (npm config set registry)
- Multi-user support — nodepilot manages a single user's Node.js environment

## Further Notes

- The app targets macOS and Windows initially. Linux is not in scope for v1.
- On Windows, the app must request administrator privileges for symlink creation. This affects the installer packaging (NSIS) and the app manifest.
- The tray icon with embedded version number requires a font rendering library in Rust. Consider `imageproc` + `rusttype` for macOS (.png) and `ico` crate for Windows (.ico).
- The phone-sized panel (375×667) is an opinionated UX choice to keep the UI focused and avoid feature creep toward a full-window app. This size fits `tdesign-mobile-vue` component proportions naturally.
- The name of the base directory (`~/.nodepilot/`) follows the nvm convention of `~/.nvm/`. This may conflict if the user already uses nvm — consider detecting an existing `.nvm` on first run and warning the user.

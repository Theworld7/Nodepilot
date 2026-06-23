# nodepilot — Domain Glossary

## Domain
A Node.js version manager GUI app. Manages multiple Node.js installations, allows switching between versions, and lives in the OS system tray.

## Core Concepts

### Version Manager
The application itself — a Tauri desktop app that manages Node.js versions on behalf of the user.

### Node.js Version (or simply "Version")
A specific Node.js release (e.g. v24.1.2). Can be installed, active (currently symlinked), or available (listed on remote but not installed locally).

### Active Version
The version currently pointed to by `~/.nodepilot/current`. The one used when the user runs `node` in a terminal.

## Installation Model

### Node Pilot Directory (`~/.nodepilot/`)
Root directory for all nodepilot data. Contains `versions/` and `current`.

### Versions Directory (`~/.nodepilot/versions/`)
Houses each installed version in its own subdirectory, e.g. `~/.nodepilot/versions/v24.1.2/`.

### Current Symlink (`~/.nodepilot/current`)
Symlink pointing to one of the version directories. `~/.nodepilot/current/bin` is injected into the system PATH automatically on first launch (see Automatic Environment Setup).

## User Interface

### Tray Icon
Status bar / system tray icon showing the active Node.js major version number (e.g. "24"). Clicking opens the panel.

### Panel
A compact desktop window (500×700) containing the version list. Implemented as a Tauri window with Vue components.

### Version List
Flat list of all known Node.js versions, filterable by major version number. Shows version number, install status, release date, and LTS label.

## System Interactions

### Version Source
The remote registry from which the version list and binaries are fetched. Default is `nodejs.org`. Supports user-configurable mirror URLs.

### Version Installation
Rust backend downloads the binary archive from the remote source, extracts it (tar.gz / zip), and places it under `~/.nodepilot/versions/`. Archive is deleted after extraction.

### Version Activation
Rust backend updates the `~/.nodepilot/current` symlink to point to the target version's directory. PATH injection is handled automatically by the Automatic Environment Setup on first launch.

### Automatic Environment Setup
On first launch, the backend silently injects `~/.nodepilot/current/bin` into the system PATH via platform-specific mechanisms (macOS: launchd agent + shell rc modification; Windows: HKCU registry PATH). No manual configuration is required. Setup is tracked by `~/.nodepilot/.auto-setup-done` flag to avoid repeated attempts.

### Competing Version Manager
A pre-existing Node.js version manager on the user's system (e.g. nvm, fnm, volta, Homebrew Node). During Automatic Environment Setup, competing managers are detected and their shell initialization hooks are disabled (commented out in `.zshrc`/`.bashrc`/PowerShell Profile) to ensure nodepilot's version takes priority. These modifications are reversible on unsetup.

### Environment Rollback
If environment setup fails (e.g. file write error, invalid shell config), all modifications are automatically undone: launchd plist removed, registry entries deleted, and shell config comments reverted. A native dialog offers retry or skip.

### Global Package Migration
Upon activation of a different version, offers to reinstall the global npm packages from the previously active version.

## Technical Architecture

### Backend
Rust (Tauri backend) owns all version management logic — listing, downloading, installing, switching, deleting. Communicates progress to the UI via Tauri events.

### Frontend
Vue 3 + tdesign-vue-next in a desktop window. Communicates with Rust via Tauri IPC (invoke commands + event listeners).

### State & Caching
Version list is cached locally as JSON. On panel open, shows cached data immediately and refreshes in the background.

### Offline Behaviour
When offline, shows cached version list. Installation and refresh are disabled.

## Supported Platforms
- macOS (status bar icon via system-tray plugin, popup window)
- Windows (system tray icon, popup window, runs with admin privileges for symlink creation)

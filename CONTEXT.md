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
Symlink pointing to one of the version directories. The user adds `~/.nodepilot/current/bin` to their PATH.

## User Interface

### Tray Icon
Status bar / system tray icon showing the active Node.js major version number (e.g. "24"). Clicking opens the panel.

### Panel
A small popup window (phone-sized, ~375×667) containing the version list. Implemented as a Tauri child window with Vue components.

### Version List
Flat list of all known Node.js versions, filterable by major version number. Shows version number, install status, release date, and LTS label.

## System Interactions

### Version Source
The remote registry from which the version list and binaries are fetched. Default is `nodejs.org`. Supports user-configurable mirror URLs.

### Version Installation
Rust backend downloads the binary archive from the remote source, extracts it (tar.gz / zip), and places it under `~/.nodepilot/versions/`. Archive is deleted after extraction.

### Version Activation
Rust backend updates the `~/.nodepilot/current` symlink to point to the target version's directory. User's PATH must include `~/.nodepilot/current/bin` (configured manually via setup guide).

### Global Package Migration
Upon activation of a different version, offers to reinstall the global npm packages from the previously active version.

## Technical Architecture

### Backend
Rust (Tauri backend) owns all version management logic — listing, downloading, installing, switching, deleting. Communicates progress to the UI via Tauri events.

### Frontend
Vue 3 + tdesign-mobile-vue in a phone-sized popup window. Communicates with Rust via Tauri IPC (invoke commands + event listeners).

### State & Caching
Version list is cached locally as JSON. On panel open, shows cached data immediately and refreshes in the background.

### Offline Behaviour
When offline, shows cached version list. Installation and refresh are disabled.

## Supported Platforms
- macOS (status bar icon via system-tray plugin, popup window)
- Windows (system tray icon, popup window, runs with admin privileges for symlink creation)

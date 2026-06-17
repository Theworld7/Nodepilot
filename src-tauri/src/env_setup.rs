use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
#[allow(dead_code)]
pub enum EnvSetupError {
    Io(String),
    Plist(String),
    Registry(String),
    ShellConfig(String),
}

impl std::fmt::Display for EnvSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            Self::Plist(msg) => write!(f, "launchd plist error: {msg}"),
            Self::Registry(msg) => write!(f, "registry error: {msg}"),
            Self::ShellConfig(msg) => write!(f, "shell config error: {msg}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Competing manager detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct CompetingManager {
    pub name: &'static str,
    /// Shell config paths that may contain init hooks for this manager.
    pub config_paths: Vec<PathBuf>,
    /// Lines that were disabled (commented out) – used for rollback.
    pub disabled_lines: Vec<(PathBuf, usize, String)>,
}

/// Patterns that identify a competing manager in a shell-config line.
fn is_manager_line(name: &str, line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }
    match name {
        "nvm" => {
            trimmed.contains("NVM_DIR")
                || trimmed.contains("nvm.sh")
                || trimmed.contains("nvm use")
                || trimmed.contains("nvm default")
                || trimmed.contains("bash_completion.d/nvm")
        }
        "fnm" => {
            trimmed.contains("fnm env")
                || trimmed.contains("fnm use")
                || trimmed.contains("fnm multishell")
                || trimmed == r#"eval "$(fnm env)""#
                || trimmed.starts_with(r#"eval "$(fnm env"#)
        }
        "volta" => {
            trimmed.contains("VOLTA_HOME")
                || trimmed.contains("volta/bin")
                || trimmed.starts_with(r#"export VOLTA"#)
        }
        "brew" => {
            // Detect Homebrew-installed Node by checking if any PATH line
            // adds /opt/homebrew/bin or /usr/local/bin with node context.
            // More often there is no explicit shell init; we detect via binary symlink.
            // Shell config lines are unlikely, but handle them just in case.
            trimmed.contains("homebrew")
        }
        _ => false,
    }
}

/// Detect installed competing Node.js version managers on the system.
pub fn detect_competing_managers(home: &Path) -> Vec<CompetingManager> {
    let mut managers: Vec<CompetingManager> = Vec::new();

    // -- nvm ----------------------------------------------------------------
    if home.join(".nvm").is_dir() {
        managers.push(CompetingManager {
            name: "nvm",
            config_paths: shell_configs(home),
            disabled_lines: Vec::new(),
        });
    }

    // -- fnm ----------------------------------------------------------------
    let fnm_dirs = [
        home.join(".local/share/fnm"),
        home.join(".fnm"),
    ];
    let has_fnm_dir = fnm_dirs.iter().any(|d| d.is_dir());
    let has_fnm_bin = which_in_path("fnm");
    if has_fnm_dir || has_fnm_bin {
        managers.push(CompetingManager {
            name: "fnm",
            config_paths: shell_configs(home),
            disabled_lines: Vec::new(),
        });
    }

    // -- volta --------------------------------------------------------------
    if home.join(".volta").is_dir() {
        managers.push(CompetingManager {
            name: "volta",
            config_paths: shell_configs(home),
            disabled_lines: Vec::new(),
        });
    }

    // -- Homebrew node ------------------------------------------------------
    if has_brew_node() {
        managers.push(CompetingManager {
            name: "brew",
            config_paths: shell_configs(home),
            disabled_lines: Vec::new(),
        });
    }

    managers
}

fn shell_configs(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join(".zshrc"),
        home.join(".bashrc"),
        home.join(".bash_profile"),
        home.join(".profile"),
    ]
}

fn which_in_path(binary: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let candidate = Path::new(dir).join(binary);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

fn has_brew_node() -> bool {
    // Check common Homebrew node locations
    let candidates = [
        "/opt/homebrew/bin/node",
        "/usr/local/bin/node",
    ];
    for p in &candidates {
        let path = Path::new(p);
        if path.is_file() {
            // Verify it's a Homebrew symlink
            if let Ok(target) = fs::read_link(path) {
                let s = target.to_string_lossy();
                if s.contains("Cellar") || s.contains("homebrew") {
                    return true;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Shell config manipulation (cross-platform: disable / restore manager lines)
// ---------------------------------------------------------------------------

/// Comment out lines that match any known competing manager pattern.
pub fn disable_competing_managers(
    managers: &mut [CompetingManager],
) -> Result<(), EnvSetupError> {
    for manager in managers.iter_mut() {
        for config_path in &manager.config_paths {
            if !config_path.exists() {
                continue;
            }
            manager.disabled_lines = modify_shell_config(
                config_path,
                |line| is_manager_line(manager.name, line),
                true, // comment_out = true
            )?;
        }
    }
    Ok(())
}

/// Restore shell config lines that were previously commented out.
pub fn restore_competing_managers(
    managers: &[CompetingManager],
) -> Result<(), EnvSetupError> {
    for manager in managers {
        for (config_path, _line_idx, _original) in &manager.disabled_lines {
            if !config_path.exists() {
                continue;
            }
            // Restore by un-commenting any line that starts with `# ` and
            // would match the manager pattern after un-commenting.
            let _ = modify_shell_config(
                config_path,
                |line| {
                    let stripped = line.trim().strip_prefix("# ").unwrap_or(line);
                    is_manager_line(manager.name, stripped)
                },
                false, // uncomment
            );
        }
    }
    Ok(())
}

/// Read a shell config file line-by-line, apply a transformation
/// (comment-out or uncomment matching lines), and write back atomically.
///
/// Returns the list of (path, line_index, original_text) that were modified.
fn modify_shell_config<F>(
    config_path: &Path,
    matcher: F,
    comment_out: bool,
) -> Result<Vec<(PathBuf, usize, String)>, EnvSetupError>
where
    F: Fn(&str) -> bool,
{
    let content = fs::read_to_string(config_path)
        .map_err(|e| EnvSetupError::ShellConfig(format!(
            "read {}: {e}", config_path.display()
        )))?;

    let mut modified = Vec::new();
    let mut output = String::new();

    for (idx, line) in content.lines().enumerate() {
        if matcher(line) {
            modified.push((config_path.to_path_buf(), idx, line.to_string()));
            if comment_out {
                // Only comment if not already commented
                if !line.trim().starts_with('#') {
                    output.push_str("# ");
                    output.push_str(line);
                } else {
                    output.push_str(line);
                }
            } else {
                // Uncomment: remove leading "# " or "#"
                if let Some(rest) = line.trim().strip_prefix("# ") {
                    output.push_str(rest);
                } else if let Some(rest) = line.trim().strip_prefix('#') {
                    output.push_str(rest);
                } else {
                    output.push_str(line);
                }
            }
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }

    if !modified.is_empty() {
        let tmp = config_path.with_extension("nodepilot-tmp");
        fs::write(&tmp, &output)
            .map_err(|e| EnvSetupError::ShellConfig(format!(
                "write tmp {}: {e}", tmp.display()
            )))?;
        fs::rename(&tmp, config_path)
            .map_err(|e| EnvSetupError::ShellConfig(format!(
                "rename {} → {}: {e}", tmp.display(), config_path.display()
            )))?;
    }

    Ok(modified)
}

// ---------------------------------------------------------------------------
// macOS specific – launchd + shell PATH injection
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    /// Write a launchd agent plist that sets the PATH environment variable
    /// for GUI-launched processes (terminals, IDEs, etc.).
    pub fn install_launchd_agent(
        bin_path: &str,
        nodepilot_dir: &Path,
    ) -> Result<(), EnvSetupError> {
        let home = dirs::home_dir().ok_or_else(|| {
            EnvSetupError::Io("cannot determine home directory".into())
        })?;

        let launch_agents = home.join("Library/LaunchAgents");
        fs::create_dir_all(&launch_agents).map_err(|e| {
            EnvSetupError::Io(format!(
                "create LaunchAgents dir: {e}"
            ))
        })?;

        let plist_path = launch_agents.join("com.nodepilot.env.plist");

        // Build the full PATH value: nodepilot bin first, then existing PATH
        let existing_path =
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());

        // Put nodepilot bin at the front so it takes priority
        let new_path = format!("{bin_path}:{existing_path}");

        let mut dict = plist::Dictionary::new();
        dict.insert(
            "Label".to_string(),
            plist::Value::String("com.nodepilot.env".to_string()),
        );
        dict.insert(
            "ProgramArguments".to_string(),
            plist::Value::Array(vec![
                plist::Value::String("/bin/launchctl".to_string()),
                plist::Value::String("setenv".to_string()),
                plist::Value::String("PATH".to_string()),
                plist::Value::String(new_path),
            ]),
        );
        dict.insert(
            "RunAtLoad".to_string(),
            plist::Value::Boolean(true),
        );
        dict.insert(
            "LimitLoadToSessionType".to_string(),
            plist::Value::String("Aqua".to_string()),
        );

        let mut buf = Vec::new();
        plist::to_writer_xml(&mut buf, &plist::Value::Dictionary(dict)).map_err(|e| {
            EnvSetupError::Plist(format!("serialize plist: {e}"))
        })?;
        let xml = String::from_utf8(buf).map_err(|e| {
            EnvSetupError::Plist(format!("plist XML is not valid UTF-8: {e}"))
        })?;

        fs::write(&plist_path, xml).map_err(|e| {
            EnvSetupError::Io(format!(
                "write plist {}: {e}",
                plist_path.display()
            ))
        })?;

        // Load the agent into the current session
        let output = std::process::Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&plist_path)
            .output();

        if let Err(e) = output {
            return Err(EnvSetupError::Plist(format!(
                "launchctl load failed: {e}"
            )));
        }

        // Also write the PATH line directly into shell config files so
        // terminals that override PATH via shell init still pick it up.
        inject_path_to_shell_rcs(bin_path, &home, nodepilot_dir)?;

        Ok(())
    }

    /// Remove the launchd agent and restore shell configs.
    pub fn uninstall_launchd_agent(
        nodepilot_dir: &Path,
    ) -> Result<(), EnvSetupError> {
        let home = dirs::home_dir().ok_or_else(|| {
            EnvSetupError::Io("cannot determine home directory".into())
        })?;

        let plist_path = home
            .join("Library/LaunchAgents")
            .join("com.nodepilot.env.plist");

        if plist_path.exists() {
            // Unload first
            let _ = std::process::Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&plist_path)
                .output();

            // Remove plist file
            fs::remove_file(&plist_path).map_err(|e| {
                EnvSetupError::Io(format!(
                    "remove plist {}: {e}",
                    plist_path.display()
                ))
            })?;
        }

        // Remove the injected PATH line from shell configs
        remove_path_from_shell_rcs(&home, nodepilot_dir)?;

        Ok(())
    }

    /// Append a PATH export to the user's shell config files so that
    /// terminals (which run shell rc on startup) also get nodepilot's bin.
    fn inject_path_to_shell_rcs(
        bin_path: &str,
        home: &Path,
        _nodepilot_dir: &Path,
    ) -> Result<(), EnvSetupError> {
        let marker = "# Added by nodepilot (auto-setup)";
        let export_line = format!("export PATH=\"{bin_path}:$PATH\"  {marker}");

        for rc in &[".zshrc", ".bashrc", ".bash_profile", ".profile"] {
            let path = home.join(rc);
            if !path.exists() {
                continue;
            }

            let content = fs::read_to_string(&path)
                .map_err(|e| EnvSetupError::ShellConfig(format!(
                    "read {}: {e}", path.display()
                )))?;

            // Skip if already injected
            if content.contains(marker) {
                continue;
            }

            let mut f = fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .map_err(|e| EnvSetupError::ShellConfig(format!(
                    "open {}: {e}", path.display()
                )))?;

            // Ensure we start on a new line
            if !content.ends_with('\n') {
                writeln!(f).map_err(|e| EnvSetupError::ShellConfig(format!(
                    "write newline {}: {e}", path.display()
                )))?;
            }
            writeln!(f, "\n{export_line}").map_err(|e| {
                EnvSetupError::ShellConfig(format!(
                    "write to {}: {e}", path.display()
                ))
            })?;
        }

        Ok(())
    }

    /// Remove nodepilot-injected PATH lines from shell config files.
    fn remove_path_from_shell_rcs(
        home: &Path,
        _nodepilot_dir: &Path,
    ) -> Result<(), EnvSetupError> {
        let marker = "# Added by nodepilot (auto-setup)";

        for rc in &[".zshrc", ".bashrc", ".bash_profile", ".profile"] {
            let path = home.join(rc);
            if !path.exists() {
                continue;
            }

            let content = fs::read_to_string(&path)
                .map_err(|e| EnvSetupError::ShellConfig(format!(
                    "read {}: {e}", path.display()
                )))?;

            if !content.contains(marker) {
                continue;
            }

            let filtered: Vec<&str> = content
                .lines()
                .filter(|line| !line.contains(marker))
                .collect();

            let tmp = path.with_extension("nodepilot-tmp");
            fs::write(&tmp, filtered.join("\n") + "\n")
                .map_err(|e| EnvSetupError::ShellConfig(format!(
                    "write tmp {}: {e}", tmp.display()
                )))?;
            fs::rename(&tmp, &path)
                .map_err(|e| EnvSetupError::ShellConfig(format!(
                    "rename {} → {}: {e}", tmp.display(), path.display()
                )))?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Windows specific – registry + PowerShell Profile
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use winreg::enums::*;
    use winreg::RegKey;

    /// Append nodepilot's bin directory to the user's PATH via
    /// HKCU\Environment, and also modify the PowerShell profile.
    pub fn inject_path_windows(
        bin_path: &str,
        nodepilot_dir: &Path,
    ) -> Result<(), EnvSetupError> {
        // -- Registry -------------------------------------------------------
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let env_key = hkcu.open_subkey_with_flags(
            "Environment",
            KEY_READ | KEY_WRITE,
        ).map_err(|e| EnvSetupError::Registry(format!(
            "open HKCU\\Environment: {e}"
        )))?;

        let current_path: String = env_key
            .get_value("Path")
            .unwrap_or_default();

        // Only append if not already present
        if !current_path.contains(bin_path) {
            let new_path = if current_path.is_empty() {
                bin_path.to_string()
            } else {
                format!("{current_path};{bin_path}")
            };

            env_key.set_value("Path", &new_path).map_err(|e| {
                EnvSetupError::Registry(format!("set PATH: {e}"))
            })?;
        }

        // -- PowerShell Profile --------------------------------------------
        modify_ps_profile(bin_path, nodepilot_dir, true)?;

        Ok(())
    }

    /// Remove nodepilot's bin directory from the user PATH and restore
    /// the PowerShell profile.
    pub fn remove_path_windows(
        bin_path: &str,
        nodepilot_dir: &Path,
    ) -> Result<(), EnvSetupError> {
        // -- Registry rollback ----------------------------------------------
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(env_key) = hkcu.open_subkey_with_flags(
            "Environment",
            KEY_READ | KEY_WRITE,
        ) {
            let current_path: String = env_key
                .get_value("Path")
                .unwrap_or_default();

            if current_path.contains(bin_path) {
                let cleaned: Vec<&str> = current_path
                    .split(';')
                    .filter(|p| !p.contains(bin_path))
                    .collect();
                let new_path = cleaned.join(";");
                env_key
                    .set_value("Path", &new_path)
                    .unwrap_or(());
            }
        }

        // -- PowerShell Profile restore ------------------------------------
        modify_ps_profile(bin_path, nodepilot_dir, false)?;

        Ok(())
    }

    /// Append / remove nodepilot PATH injection in the current user's
    /// PowerShell profile.
    fn modify_ps_profile(
        bin_path: &str,
        _nodepilot_dir: &Path,
        add: bool,
    ) -> Result<(), EnvSetupError> {
        // PowerShell profile is typically at:
        //   $HOME\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1
        //   $HOME\Documents\PowerShell\Microsoft.PowerShell_profile.ps1
        let docs = dirs::document_dir().unwrap_or_default();
        let profiles = [
            docs.join("WindowsPowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
            docs.join("PowerShell")
                .join("Microsoft.PowerShell_profile.ps1"),
        ];

        let marker = "# Added by nodepilot (auto-setup)";
        let ps_line = format!(
            "$env:Path = \"{bin_path};\" + $env:Path  {marker}"
        );

        for profile_path in &profiles {
            if add {
                // Create parent dirs if needed
                if let Some(parent) = profile_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                let exists = profile_path.exists();
                let mut file = fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(profile_path)
                    .map_err(|e| EnvSetupError::ShellConfig(format!(
                        "open {}: {e}", profile_path.display()
                    )))?;

                if exists {
                    let content = fs::read_to_string(profile_path).unwrap_or_default();
                    if content.contains(marker) {
                        continue; // already done
                    }
                    if !content.ends_with('\n') {
                        writeln!(file).map_err(|e| EnvSetupError::ShellConfig(format!(
                            "newline {}: {e}", profile_path.display()
                        )))?;
                    }
                }
                writeln!(file, "{ps_line}").map_err(|e| {
                    EnvSetupError::ShellConfig(format!(
                        "write {}: {e}", profile_path.display()
                    ))
                })?;
            } else {
                // Remove the injected line
                if !profile_path.exists() {
                    continue;
                }
                let content = fs::read_to_string(profile_path)
                    .map_err(|e| EnvSetupError::ShellConfig(format!(
                        "read {}: {e}", profile_path.display()
                    )))?;

                if !content.contains(marker) {
                    continue;
                }

                let filtered: Vec<&str> = content
                    .lines()
                    .filter(|line| !line.contains(marker))
                    .collect();

                fs::write(profile_path, filtered.join("\n") + "\n")
                    .map_err(|e| EnvSetupError::ShellConfig(format!(
                        "write {}: {e}", profile_path.display()
                    )))?;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Perform automatic environment setup on first launch.
///
/// 1. Detect competing managers (nvm/fnm/volta/brew)
/// 2. Inject PATH via platform-specific mechanism
/// 3. Disable competing manager init hooks in shell configs
/// 4. Write `.auto-setup-done` flag
pub fn setup(nodepilot_dir: &Path) -> Result<(), EnvSetupError> {
    let home = dirs::home_dir().unwrap_or_default();
    let bin_path = nodepilot_dir.join("current").join("bin");

    // Ensure we have something to display even if the bin dir doesn't exist
    // yet (it will after first install).
    let bin_str = bin_path.to_string_lossy().to_string();

    // Step 1: detect competing managers
    let managers = detect_competing_managers(&home);

    // Step 2: platform-specific PATH injection
    #[cfg(target_os = "macos")]
    macos::install_launchd_agent(&bin_str, nodepilot_dir)?;

    #[cfg(target_os = "windows")]
    windows::inject_path_windows(&bin_str, nodepilot_dir)?;

    // Step 3: disable competing managers in shell configs
    let mut managers_mut = managers;
    disable_competing_managers(&mut managers_mut)?;

    // Step 4: write done flag
    let flag_path = nodepilot_dir.join(".auto-setup-done");
    fs::write(&flag_path, b"1").map_err(|e| {
        EnvSetupError::Io(format!("write .auto-setup-done: {e}"))
    })?;

    Ok(())
}

/// Roll back ALL changes made by `setup()`.
pub fn rollback(nodepilot_dir: &Path) {
    let home = dirs::home_dir().unwrap_or_default();

    // Re-detect so we know which files to restore
    let managers = detect_competing_managers(&home);

    // Platform-specific removal
    #[cfg(target_os = "macos")]
    {
        let _ = macos::uninstall_launchd_agent(nodepilot_dir);
    }

    #[cfg(target_os = "windows")]
    {
        let bin_path = nodepilot_dir.join("current").join("bin");
        let _ = windows::remove_path_windows(
            &bin_path.to_string_lossy(),
            nodepilot_dir,
        );
    }

    // Restore shell config lines
    let _ = restore_competing_managers(&managers);

    // Remove the done flag
    let flag_path = nodepilot_dir.join(".auto-setup-done");
    let _ = fs::remove_file(&flag_path);
}

/// Check whether auto-setup has already been performed.
pub fn is_setup_done(nodepilot_dir: &Path) -> bool {
    nodepilot_dir.join(".auto-setup-done").exists()
}

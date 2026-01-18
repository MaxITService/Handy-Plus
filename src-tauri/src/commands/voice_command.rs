//! Voice Command Tauri commands
//!
//! Commands for executing voice-triggered scripts after user confirmation.
//! Works like Windows Run dialog (Win+R) with optional ${command} variable.

use log::{debug, error, info};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NEW_CONSOLE: u32 = 0x00000010;

/// Executes a command template after user confirmation.
/// Works like Windows Run dialog - executes the full command line directly.
///
/// Parameters:
/// - `command`: The script from the voice command card (replaces ${command} in template)
/// - `template`: The full command template (e.g., "powershell -Command \"${command}\"")
/// - `keep_window_open`: If true, opens a visible console window instead of silent execution
///
/// Returns the output on success or an error message on failure.
/// When `keep_window_open` is true, returns success immediately (no output capture).
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub fn execute_voice_command(
    command: String,
    template: String,
    keep_window_open: bool,
) -> Result<String, String> {
    // Replace ${command} placeholder with actual command (can be empty)
    let full_command = template.replace("${command}", &command);

    if full_command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    info!("Executing voice command: {}", full_command);
    debug!("Template: '{}', Command: '{}', keep_window_open: {}", template, command, keep_window_open);

    if keep_window_open {
        // Open a visible console window that stays open
        // Detect shell type and use appropriate "stay open" flag
        info!("Opening command in new console window: {}", full_command);

        let full_command_lower = full_command.to_lowercase();

        if full_command_lower.starts_with("powershell") {
            // PowerShell: extract args and command, run with -NoExit
            if let Some((pre_args, cmd_content)) = parse_powershell_command(&full_command) {
                debug!("PowerShell -NoExit with args {:?}, command: {}", pre_args, cmd_content);
                let mut cmd = Command::new("powershell");
                cmd.args(&pre_args);
                cmd.args(["-NoExit", "-Command", &cmd_content]);
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                cmd.spawn().map_err(|e| format!("Failed to open PowerShell window: {}", e))?;
            } else {
                // Fallback: run the whole command via cmd /k
                let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
                Command::new(&comspec)
                    .args(["/k", &full_command])
                    .creation_flags(CREATE_NEW_CONSOLE)
                    .spawn()
                    .map_err(|e| format!("Failed to open console window: {}", e))?;
            }
        } else if full_command_lower.starts_with("pwsh") {
            // PowerShell 7+: extract args and command, run with -NoExit
            if let Some((pre_args, cmd_content)) = parse_powershell_command(&full_command) {
                debug!("pwsh -NoExit with args {:?}, command: {}", pre_args, cmd_content);
                let mut cmd = Command::new("pwsh");
                cmd.args(&pre_args);
                cmd.args(["-NoExit", "-Command", &cmd_content]);
                cmd.creation_flags(CREATE_NEW_CONSOLE);
                cmd.spawn().map_err(|e| format!("Failed to open pwsh window: {}", e))?;
            } else {
                let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
                Command::new(&comspec)
                    .args(["/k", &full_command])
                    .creation_flags(CREATE_NEW_CONSOLE)
                    .spawn()
                    .map_err(|e| format!("Failed to open console window: {}", e))?;
            }
        } else {
            // Generic command: use cmd /k
            let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
            Command::new(&comspec)
                .args(["/k", &full_command])
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn()
                .map_err(|e| format!("Failed to open console window: {}", e))?;
        }

        Ok("Command opened in console window".to_string())
    } else {
        // Silent execution via cmd /c (like Win+R)
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".to_string());
        debug!("Silent execution via {}: /c {}", comspec, full_command);

        let output = Command::new(&comspec)
            .args(["/c", &full_command])
            .output()
            .map_err(|e| format!("Failed to execute command: {}", e))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() {
            debug!("Command executed successfully. Output: {}", stdout.trim());
            Ok(stdout)
        } else {
            error!("Command failed. Stderr: {}", stderr);
            Err(format!("Command failed: {}", stderr.trim()))
        }
    }
}

/// Parse PowerShell command line and extract pre-command args and command content.
/// E.g., `powershell -NoProfile -Command "start msedge"` → (vec!["-NoProfile"], "start msedge")
#[cfg(target_os = "windows")]
fn parse_powershell_command(full_cmd: &str) -> Option<(Vec<String>, String)> {
    let lower = full_cmd.to_lowercase();

    // Find -Command or -c (short form)
    let cmd_idx = lower.find("-command ").or_else(|| lower.find("-c "))?;

    // Get args between shell name and -Command
    let shell_end = if lower.starts_with("powershell.exe") {
        14
    } else if lower.starts_with("powershell") {
        10
    } else if lower.starts_with("pwsh.exe") {
        8
    } else if lower.starts_with("pwsh") {
        4
    } else {
        0
    };

    let pre_args_str = full_cmd[shell_end..cmd_idx].trim();
    let pre_args: Vec<String> = pre_args_str
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    // Get everything after -Command/-c
    let after_flag = if lower[cmd_idx..].starts_with("-command ") {
        &full_cmd[cmd_idx + 9..] // len("-command ") = 9
    } else {
        &full_cmd[cmd_idx + 3..] // len("-c ") = 3
    };

    let trimmed = after_flag.trim();

    // Remove surrounding quotes if present
    let cmd_content = if (trimmed.starts_with('"') && trimmed.ends_with('"')) ||
       (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
        trimmed[1..trimmed.len()-1].to_string()
    } else {
        trimmed.to_string()
    };

    Some((pre_args, cmd_content))
}

/// Find Windows Terminal (wt.exe) by checking multiple locations.
/// Returns the path to wt.exe if found, or an error with helpful message.
#[cfg(target_os = "windows")]
fn find_windows_terminal() -> Result<String, String> {
    // First try: just "wt" (relies on PATH)
    if let Ok(output) = Command::new("where").arg("wt.exe").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout);
            if let Some(first_line) = path.lines().next() {
                let trimmed = first_line.trim();
                if !trimmed.is_empty() {
                    debug!("Found wt.exe via PATH: {}", trimmed);
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    // Second try: WindowsApps in LOCALAPPDATA (user app execution alias)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let windows_apps_path = format!("{}\\Microsoft\\WindowsApps\\wt.exe", local_app_data);
        if std::path::Path::new(&windows_apps_path).exists() {
            debug!("Found wt.exe in WindowsApps: {}", windows_apps_path);
            return Ok(windows_apps_path);
        }
    }

    // Third try: Check common Program Files locations (for non-Store installs)
    let program_files_paths = [
        "C:\\Program Files\\Windows Terminal\\wt.exe",
        "C:\\Program Files (x86)\\Windows Terminal\\wt.exe",
    ];
    for path in program_files_paths {
        if std::path::Path::new(path).exists() {
            debug!("Found wt.exe in Program Files: {}", path);
            return Ok(path.to_string());
        }
    }

    Err(
        "Windows Terminal (wt.exe) not found. Please ensure Windows Terminal is installed:\n\
         1. Check Start Menu for 'Windows Terminal'\n\
         2. Install from Microsoft Store"
            .to_string(),
    )
}

/// Non-Windows stub
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub fn execute_voice_command(
    _command: String,
    _template: String,
    _keep_window_open: bool,
) -> Result<String, String> {
    Err("Voice commands are only supported on Windows".to_string())
}

/// Tests voice command matching with mock text (simulates STT output).
/// Runs the same matching logic as if the text was spoken.
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub async fn test_voice_command_mock(
    app: tauri::AppHandle,
    mock_text: String,
) -> Result<String, String> {
    use crate::actions::{find_matching_command, generate_command_with_llm, CommandConfirmPayload, FuzzyMatchConfig};
    use crate::settings::get_settings;
    use log::debug;

    if mock_text.trim().is_empty() {
        return Err("Mock text is empty".to_string());
    }

    info!("Testing voice command with mock text: '{}'", mock_text);

    let settings = get_settings(&app);
    let fuzzy_config = FuzzyMatchConfig::from_settings(&settings);

    // Step 1: Try to match against predefined commands
    if let Some((matched_cmd, score)) = find_matching_command(
        &mock_text,
        &settings.voice_commands,
        settings.voice_command_default_threshold,
        &fuzzy_config,
    ) {
        debug!(
            "Mock test matched: '{}' -> '{}' (score: {:.2})",
            matched_cmd.trigger_phrase, matched_cmd.script, score
        );

        // Show confirmation overlay
        crate::overlay::show_command_confirm_overlay(
            &app,
            CommandConfirmPayload {
                command: matched_cmd.script.clone(),
                spoken_text: mock_text.clone(),
                from_llm: false,
                template: settings.voice_command_template.clone(),
                keep_window_open: settings.voice_command_keep_window_open,
                auto_run: settings.voice_command_auto_run,
                auto_run_seconds: settings.voice_command_auto_run_seconds,
            },
        );

        return Ok(format!(
            "Matched predefined command: '{}' (score: {:.0}%)",
            matched_cmd.name,
            score * 100.0
        ));
    }

    // Step 2: No predefined match - try LLM fallback if enabled
    if settings.voice_command_llm_fallback {
        debug!(
            "No predefined match, using LLM fallback for mock text: '{}'",
            mock_text
        );

        match generate_command_with_llm(&app, &mock_text).await {
            Ok(suggested_command) => {
                debug!("LLM suggested command: '{}'", suggested_command);

                // Show confirmation overlay
                crate::overlay::show_command_confirm_overlay(
                    &app,
                    CommandConfirmPayload {
                        command: suggested_command.clone(),
                        spoken_text: mock_text,
                        from_llm: true,
                        template: settings.voice_command_template.clone(),
                        keep_window_open: settings.voice_command_keep_window_open,
                        auto_run: false, // Never auto-run LLM-generated commands
                        auto_run_seconds: 0,
                    },
                );

                return Ok(format!("LLM generated command: '{}'", suggested_command));
            }
            Err(e) => {
                return Err(format!("LLM fallback failed: {}", e));
            }
        }
    }

    Err(format!(
        "No matching command found for: '{}' (LLM fallback disabled)",
        mock_text
    ))
}

/// Non-Windows stub for mock testing
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub async fn test_voice_command_mock(
    _app: tauri::AppHandle,
    _mock_text: String,
) -> Result<String, String> {
    Err("Voice commands are only supported on Windows".to_string())
}

//! Voice Command Tauri commands
//!
//! Commands for executing voice-triggered scripts after user confirmation.

use log::{debug, error, info};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::Command;

#[cfg(target_os = "windows")]
const CREATE_NEW_CONSOLE: u32 = 0x00000010;

#[cfg(target_os = "windows")]
fn parse_ps_args(ps_args: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = ps_args.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                if matches!(chars.peek(), Some('\'')) {
                    current.push('\'');
                    chars.next();
                } else {
                    in_single = false;
                }
            } else {
                current.push(ch);
            }
            continue;
        }

        if in_double {
            match ch {
                '"' => {
                    in_double = false;
                }
                '`' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    } else {
                        current.push('`');
                    }
                }
                _ => current.push(ch),
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '`' => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('`');
                }
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

/// Executes a PowerShell command after user confirmation.
///
/// Parameters:
/// - `command`: The PowerShell command to execute
/// - `ps_args`: PowerShell arguments (e.g., "-NoProfile -NonInteractive")
/// - `keep_window_open`: If true, opens a visible terminal window instead of silent execution
/// - `use_windows_terminal`: If true, uses Windows Terminal (wt); otherwise uses classic PowerShell window
/// - `use_pwsh`: If true, uses PowerShell 7+ (pwsh); otherwise uses Windows PowerShell 5.1 (powershell)
///
/// Returns the output on success or an error message on failure.
/// When `keep_window_open` is true, returns success immediately (no output capture).
#[tauri::command]
#[specta::specta]
#[cfg(target_os = "windows")]
pub fn execute_voice_command(
    command: String,
    ps_args: String,
    keep_window_open: bool,
    use_windows_terminal: bool,
    use_pwsh: bool,
) -> Result<String, String> {
    if command.trim().is_empty() {
        return Err("Command is empty".to_string());
    }

    // Determine which PowerShell to use
    let ps_executable = if use_pwsh { "pwsh" } else { "powershell" };

    info!("Executing voice command: {}", command);
    debug!(
        "Options: ps_args='{}', keep_window_open={}, use_windows_terminal={}, use_pwsh={}, shell={}",
        ps_args, keep_window_open, use_windows_terminal, use_pwsh, ps_executable
    );

    if keep_window_open {
        // Open a visible terminal window that stays open after command completes
        // Use -NoExit to keep the window open
        let mut ps_args_vec = parse_ps_args(&ps_args);
        if !ps_args_vec
            .iter()
            .any(|arg| arg.eq_ignore_ascii_case("-NoExit"))
        {
            ps_args_vec.push("-NoExit".to_string());
        }

        if use_windows_terminal {
            // Use Windows Terminal (wt) with PowerShell
            // wt new-tab powershell <args> -Command "<command>"
            let wt_path = find_windows_terminal()?;

            let mut display_args = vec![
                "new-tab".to_string(),
                "--".to_string(),
                ps_executable.to_string(),
            ];
            display_args.extend(ps_args_vec.clone());
            display_args.push("-Command".to_string());
            display_args.push(command.clone());

            info!(
                "Opening Windows Terminal: {} {}",
                wt_path,
                display_args.join(" ")
            );

            Command::new(&wt_path)
                .arg("new-tab")
                .arg("--")
                .arg(ps_executable)
                .args(&ps_args_vec)
                .arg("-Command")
                .arg(&command)
                .spawn()
                .map_err(|e| format!("Failed to open Windows Terminal: {}", e))?;
        } else {
            // Use classic PowerShell window by spawning a new console
            info!("Opening {} window in a new console", ps_executable);

            Command::new(ps_executable)
                .creation_flags(CREATE_NEW_CONSOLE)
                .args(&ps_args_vec)
                .arg("-Command")
                .arg(&command)
                .spawn()
                .map_err(|e| format!("Failed to open {} window: {}", ps_executable, e))?;
        }

        Ok("Command opened in terminal window".to_string())
    } else {
        // Silent execution with output capture (original behavior)
        let ps_args_vec = parse_ps_args(&ps_args);

        let mut cmd = Command::new(ps_executable);
        cmd.args(&ps_args_vec);
        cmd.arg("-Command").arg(&command);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to spawn {}: {}", ps_executable, e))?;

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
        "Windows Terminal (wt.exe) not found. Please ensure Windows Terminal is installed and try:\n\
         1. Check Start Menu for 'Windows Terminal'\n\
         2. Install from Microsoft Store\n\
         3. Or disable 'Use Windows Terminal' to use classic PowerShell window"
            .to_string(),
    )
}

/// Non-Windows stub
#[tauri::command]
#[specta::specta]
#[cfg(not(target_os = "windows"))]
pub fn execute_voice_command(
    _command: String,
    _ps_args: String,
    _keep_window_open: bool,
    _use_windows_terminal: bool,
    _use_pwsh: bool,
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
    use crate::actions::{find_matching_command, generate_command_with_llm, CommandConfirmPayload};
    use crate::settings::get_settings;
    use log::debug;

    if mock_text.trim().is_empty() {
        return Err("Mock text is empty".to_string());
    }

    info!("Testing voice command with mock text: '{}'", mock_text);

    let settings = get_settings(&app);

    // Step 1: Try to match against predefined commands
    if let Some((matched_cmd, score)) = find_matching_command(
        &mock_text,
        &settings.voice_commands,
        settings.voice_command_default_threshold,
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
                ps_args: settings.voice_command_ps_args.clone(),
                keep_window_open: settings.voice_command_keep_window_open,
                use_windows_terminal: settings.voice_command_use_windows_terminal,
                use_pwsh: settings.voice_command_use_pwsh,
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
                        ps_args: settings.voice_command_ps_args.clone(),
                        keep_window_open: settings.voice_command_keep_window_open,
                        use_windows_terminal: settings.voice_command_use_windows_terminal,
                        use_pwsh: settings.voice_command_use_pwsh,
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

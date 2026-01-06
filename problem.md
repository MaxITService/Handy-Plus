# Deep Research Request: Tauri v2 Windows MSI Signing Issues

## Context

I am building a **Tauri v2** application for Windows (MSI target only) using **GitHub Actions**. I am attempting to implement a custom `signCommand` to sign the generated `.exe` binaries and the final `.msi` package using a PFX certificate.

## The Problem

Despite the build succeeding and the logs showing "Signing...", the resulting artifacts are **not signed**. Additionally, attempts to move the signing logic into a PowerShell script for better debugging have caused the build to fail with the error `failed to bundle project: failed to run powershell`.

## Technical Specs

- **Framework**: Tauri v2
- **Language**: Rust / TypeScript (Frontend)
- **CI/CD**: GitHub Actions (`windows-latest` runner)
- **Tooling**: `signtool.exe` (Windows SDK), `powershell.exe` (5.1), `pwsh` (7+)
- **Target**: MSI Bundle

---

## Part 1: Initial Failure (Silent Non-Signing)

### Configuration:

Inside `tauri.conf.json`:

```json
"bundle": {
  "windows": {
    "signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -Command \"signtool sign /f $env:PFX_PATH /p $env:PFX_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /v %1\""
  }
}
```

### Observation:

The build logs show:

```
Signing Output of signing command:
signtool sign /f D:\a\_temp\codesign.pfx /p *** /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /v %1
```

**Critical Clue**: The `%1` placeholder is being printed literally in the "Signing Output". It appears it was never substituted by Tauri, or it was incorrectly escaped inside the PowerShell `-Command` string. The resulting binary is unsigned.

---

## Part 2: Attempted Fix (External Script)

To debug why `%1` wasn't substituting and to add `signtool verify` logic, we moved to an external script `src-tauri/scripts/sign-windows.ps1`.

### New Configuration:

```json
"signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\sign-windows.ps1 \"%1\""
```

### Script Logic (`sign-windows.ps1`):

It uses `param($FilePath)` and calls `signtool sign` then `signtool verify`.

### Result: Build Failure

Error: `failed to bundle project: failed to run powershell` (or `failed to run pwsh` when we tried that).
This suggests the process spawner in Tauri v2 is failing to even start the shell.

---

## Part 3: Hypotheses to Investigate

I need a deep research agent to look into the following:

1.  **Tauri v2 `signCommand` Tokenization**: How does the Tauri v2 bundler (specifically for Windows MSI) split the `signCommand` string? Does it use `std::process::Command` in a way that breaks when spaces or escaped quotes are used in arguments?
2.  **Substitution Order**: At what exact stage does `%1` get substituted? If the command is passed to a shell, do the quotes around `%1` (e.g., `"%1"`) cause issues with Tauri's internal regex/substitution logic?
3.  **Working Directory Context**: When `signCommand` is executed on GitHub Actions, what is the current working directory? Is it the project root or `src-tauri`? If we use a relative path like `scripts\sign-windows.ps1`, is it relative to where the binary sits or the config file?
4.  **Shell Execution Policy**: Is there a known issue with Tauri v2 spawning `powershell.exe` specifically on GitHub Actions runners?
5.  **Best Practice for Tauri v2 + Signtool**: Search for successful implementations of custom signing commands in Tauri v2. Is there a preferred way to handle paths with spaces or complex arguments (e.g., using a `.bat` wrapper or absolute paths)?
6.  **`failed to run powershell` Error**: This specific error code in the Rust `tauri-bundle` crate — what are the possible root causes? (E.g., executable not found, permission denied, or invalid argument string).

---

## Goal

Provide a definitive, battle-tested `signCommand` configuration and script structure that:

1.  Correctly substitutes the file path.
2.  Actually executes `signtool`.
3.  Handles potential spaces in the file path (which `%1` often contains).
4.  Does not crash the Tauri bundler's process spawner. + try to find multiple places where actual build code succeeded.

---

## Part 4: All Attempted Configurations (Chronological)

### Attempt #0: Original (Before Any Fix)

```json
"signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -Command \"signtool sign /f $env:PFX_PATH /p $env:PFX_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /v %1\""
```

**Result:** PowerShell DID run. Environment variables expanded. But `%1` was NOT substituted (appeared literally in output). Binary unsigned.

**Analysis:** `%1` was inside the quoted `-Command` string, so Tauri's regex didn't find it.

---

### Attempt #1: External Script with pwsh

```json
"signCommand": "pwsh -NoProfile -ExecutionPolicy Bypass -File scripts\\sign-windows.ps1 \"%1\""
```

**Result:** `failed to bundle project: failed to run pwsh`

**Analysis:** `pwsh` (PowerShell 7) not found in Tauri's spawn context.

---

### Attempt #2: External Script with powershell

```json
"signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -File scripts\\sign-windows.ps1 \"%1\""
```

**Result:** `failed to bundle project: failed to run powershell`

**Analysis:** Unclear why PowerShell fails here when it worked in Attempt #0. Possibly:

- Path `scripts\\` is wrong (should be `src-tauri\\scripts\\`)
- The `\"%1\"` quoting breaks Tauri's tokenization

---

### Attempt #3: Object Syntax with powershell

```json
"signCommand": {
  "cmd": "powershell",
  "args": [
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", "src-tauri\\scripts\\sign-windows.ps1",
    "%1"
  ]
}
```

**Result:** `failed to bundle project: failed to run powershell`

**Analysis:** Object syntax recommended by research, but still fails. Tauri's process spawner cannot start PowerShell.

---

### Attempt #4: Object Syntax with Full Path to PowerShell

```json
"signCommand": {
  "cmd": "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
  "args": [
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", "src-tauri\\scripts\\sign-windows.ps1",
    "%1"
  ]
}
```

**Result:** `failed to bundle project: failed to run C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`

**Analysis:** Even with absolute path, spawn fails. Problem is not "finding" PowerShell.

---

### Attempt #5: Object Syntax with CMD

```json
"signCommand": {
  "cmd": "cmd",
  "args": ["/C", "powershell -NoProfile -ExecutionPolicy Bypass -File src-tauri\\scripts\\sign-windows.ps1 %1"]
}
```

**Result:** `failed to bundle project: failed to run cmd`

**Analysis:** Object syntax appears fundamentally broken. Even `cmd.exe` fails to spawn.

---

### Attempt #6: String Format with -File and Forward Slashes

```json
"signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -File src-tauri/scripts/sign-windows.ps1 %1"
```

**Result:** `failed to bundle project: failed to run powershell`

**Analysis:** String format with `-File` doesn't work, even though string format with `-Command` DID work in Attempt #0.

---

### Attempt #7: String Format with -Command and $args[0] Trick

```json
"signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -Command \"& 'src-tauri/scripts/sign-windows.ps1' $args[0]\" %1"
```

**Result:** `failed to bundle project: failed to run powershell`

**Analysis:** Even this approach failed. The `-Command` format which worked in Attempt #0 now fails. The difference is that Attempt #0 had NO external script call — it was all inline.

---

## Key Observations

1. **String format with `-Command` and INLINE code WORKS** (Attempt #0 ran PowerShell successfully)
2. **String format with `-File` ALWAYS FAILS** (Attempts #2, #6)
3. **String format with `-Command` calling external script FAILS** (Attempt #7)
4. **Object/array syntax ALWAYS FAILS** (Attempts #3, #4, #5) — even with `cmd.exe`
5. **The `%1` must be OUTSIDE any quoted strings** for Tauri to substitute it
6. **Working directory is project root**, not `src-tauri`

---

## Part 5: Real-World Observations from GitHub Actions

### What DOES Work in GitHub Actions

1. **PowerShell in workflow `run:` steps works perfectly:**
   - The "Restore PFX from secret" step uses `shell: pwsh` and executes complex PowerShell code
   - Output shows: "RESTORE PFX CERTIFICATE - DEBUG", PFX decoded, file written, env vars set
   - This proves PowerShell 7 (`pwsh`) is installed and functional

2. **The build itself completes:**
   - Rust compilation: ✅
   - Frontend build: ✅
   - Binary creation: ✅ (`aivorelay.exe` is built)
   - The failure happens ONLY at the signing step

3. **Environment variables ARE set:**
   - `PFX_PATH` and `PFX_PASSWORD` are exported to `GITHUB_ENV`
   - The original Attempt #0 DID expand `$env:PFX_PATH` correctly in the logs

### What FAILS

1. **Any signCommand that includes `-File` argument**
2. **Any signCommand using object/array syntax (`cmd` + `args`)**
3. **Any signCommand that references an external script**

### Critical Insight

The ONLY configuration that successfully spawned PowerShell was:

```json
"signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -Command \"signtool sign ... %1\""
```

This is **fully inline** — no external files. The moment we add `-File` or reference a `.ps1` script, the spawn fails.

**Hypothesis:** Tauri's process spawner has a bug or limitation where:

- Simple inline commands work
- Any command involving file paths in arguments breaks the spawn

---

## Part 6: Possible Root Causes to Investigate

### 1. Tauri Bundler Source Code

- Repository: https://github.com/tauri-apps/tauri
- Relevant file: `crates/tauri-bundler/src/bundle/windows/msi.rs` (or `sign.rs`)
- Look for: How `signCommand` is parsed, how `%1` is substituted, how `std::process::Command` is called

### 2. Known Issues

- Search Tauri GitHub Issues for: "signCommand", "failed to run", "signing windows"
- Check if there's a regression in Tauri 2.9.x

### 3. The "failed to run X" Error

- This error likely comes from Rust's `std::process::Command::spawn()` returning an error
- Possible causes:
  - Executable not found (but we tried full paths)
  - Invalid arguments (likely — something in argument parsing)
  - Permission denied (unlikely on GitHub runner)
  - Working directory issue

### 4. Debug via SSH

- Add `mxschmitt/action-tmate` action to get interactive SSH access to the runner
- Manually test: `powershell -NoProfile -File src-tauri/scripts/sign-windows.ps1 "test.exe"`
- See if it works outside of Tauri's context

---

## Part 7: Next Steps

### Immediate Options

1. **Revert to Working Inline Command** — go back to Attempt #0 format, but put `%1` OUTSIDE quotes:

   ```json
   "signCommand": "powershell -NoProfile -ExecutionPolicy Bypass -Command \"signtool sign /f $env:PFX_PATH /p $env:PFX_PASSWORD /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 /v\" %1"
   ```

   (Note: This may not work because signtool expects the file path as part of the command, not after)

2. **SSH Debug Session** — add tmate to workflow, get shell access, test manually

3. **Read Tauri Source** — find exactly what "failed to run" means in the bundler code

4. **Search Tauri Issues** — someone else may have hit this exact problem

### Long-term Options

1. **File a Tauri Bug** — if this is a genuine bundler issue
2. **Use Azure SignTool or other approach** — some users report success with different signing tools
3. **Sign in a separate workflow step** — skip Tauri's signCommand entirely, sign after build completes

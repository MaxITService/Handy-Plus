# Fork Code Notes

Files that differentiate this fork from the original [cjpais/Handy](https://github.com/cjpais/Handy).

## New Files (Fork-Specific)

### Backend (Rust)

| File | Purpose |
|------|---------|
| `src-tauri/src/connector.rs` | HTTP client for sending transcriptions to external services (Handy Connector). Sends JSON payload `{text, ts}` to configurable endpoint. |
| `src-tauri/src/managers/remote_stt.rs` | Remote Speech-to-Text manager. Handles OpenAI-compatible API calls, WAV encoding, API key storage (Windows Credential Manager), debug logging. |
| `src-tauri/src/commands/remote_stt.rs` | Tauri commands exposing Remote STT functionality to frontend: `remote_stt_has_api_key`, `remote_stt_set_api_key`, `remote_stt_test_connection`, etc. |

### Frontend (React/TypeScript)

| File | Purpose |
|------|---------|
| `src/components/settings/remote-stt/RemoteSttSettings.tsx` | UI for Remote STT configuration: base URL, model ID, API key management, connection testing, debug log viewer. |
| `src/components/settings/advanced/AiReplaceSettings.tsx` | UI for AI Replace feature: system/user prompts, max chars limit, "no selection" mode toggle. |

## Modified Files

### Backend Core Logic

| File | Changes |
|------|---------|
| `src-tauri/src/actions.rs` | Added new shortcut actions: `AiReplaceSelectionAction`, `SendToExtensionAction`, `SendToExtensionWithSelectionAction`. These handle the new voice-to-LLM and connector workflows. |
| `src-tauri/src/settings.rs` | Extended `AppSettings` with: `transcription_provider`, `remote_stt` settings, `ai_replace_*` fields, `connector_*` fields. Added `RemoteSttSettings`, `TranscriptionProvider` enum. |
| `src-tauri/src/lib.rs` | Registered new managers (`RemoteSttManager`) and commands. |
| `src-tauri/src/shortcut.rs` | Added shortcut bindings for new actions (AI Replace, Send to Extension). |
| `src-tauri/src/clipboard.rs` | Enhanced clipboard handling for AI Replace selection capture. |
| `src-tauri/src/input.rs` | Added selection capture utilities for Windows. |

### Backend Support

| File | Changes |
|------|---------|
| `src-tauri/src/commands/mod.rs` | Exported new `remote_stt` commands module. |
| `src-tauri/src/managers/mod.rs` | Exported `remote_stt` manager module. |
| `src-tauri/src/audio_toolkit/mod.rs` | Added `encode_wav_bytes()` for Remote STT API. |
| `src-tauri/src/audio_toolkit/audio/utils.rs` | WAV encoding utilities. |
| `src-tauri/Cargo.toml` | Added dependencies: `keyring` (credential storage), `reqwest` features. |
| `src-tauri/resources/default_settings.json` | Default values for new settings. |

### Frontend Settings UI

| File | Changes |
|------|---------|
| `src/components/settings/advanced/AdvancedSettings.tsx` | Added Remote STT and AI Replace settings sections. |
| `src/components/settings/general/GeneralSettings.tsx` | Minor adjustments for new settings layout. |
| `src/components/Sidebar.tsx` | Navigation for new settings sections. |
| `src/hooks/useSettings.ts` | Hooks for new settings: `setTranscriptionProvider`, `updateRemoteStt*`, `updateAiReplace*`. |
| `src/stores/settingsStore.ts` | State management for new settings. |
| `src/i18n/locales/en/translation.json` | Translations for all new UI strings. |
| `src/bindings.ts` | Auto-generated Tauri command bindings (includes remote_stt commands). |

### Other Modified

| File | Changes |
|------|---------|
| `src/App.tsx` | Event listeners for new features (remote-stt-error, ai-replace-error). |
| `src/components/model-selector/*` | Adjusted for transcription provider switching. |
| `src/components/onboarding/Onboarding.tsx` | Updated for Remote STT option. |

## Feature → File Mapping

### Remote STT API
```
User configures in UI
    └─► RemoteSttSettings.tsx
            └─► useSettings.ts → settings.rs
                    └─► remote_stt.rs (manager)
                            └─► OpenAI-compatible API
```

### AI Replace Selection
```
User presses shortcut + speaks instruction
    └─► shortcut.rs → actions.rs (AiReplaceSelectionAction)
            └─► input.rs (capture selection)
            └─► transcription (local or remote)
            └─► llm_client.rs → LLM API
            └─► clipboard.rs (paste result)
```

### Send to Extension (Connector)
```
User presses shortcut + speaks
    └─► shortcut.rs → actions.rs (SendToExtensionAction)
            └─► transcription
            └─► connector.rs → HTTP POST to Handy Connector
```

## Entry Points for Common Tasks

| Task | Start Here |
|------|------------|
| Change AI Replace behavior | `actions.rs` → `AiReplaceSelectionAction::stop()` |
| Add new AI Replace setting | `settings.rs` → add field, `AiReplaceSettings.tsx` → add UI |
| Change message format for Connector | `actions.rs` → `build_extension_message()` |
| Change Remote STT API handling | `managers/remote_stt.rs` → `transcribe()` |
| Add new shortcut action | `actions.rs` → impl `ShortcutAction`, register in `ACTION_MAP` |
| Change selection capture logic | `input.rs` (Windows-specific) |
| Add new Tauri command | `commands/*.rs` → add fn, `commands/mod.rs` → export |

## Key Data Structures

| Structure | File | Purpose |
|-----------|------|---------|
| `AppSettings` | `settings.rs` | All app settings, includes `ai_replace_*`, `remote_stt`, `connector_*` |
| `RemoteSttSettings` | `settings.rs` | base_url, model_id, debug_mode, debug_capture |
| `TranscriptionProvider` | `settings.rs` | Enum: `Local`, `RemoteOpenAiCompatible` |
| `ShortcutAction` trait | `actions.rs` | Interface for all shortcut actions (start/stop) |
| `ACTION_MAP` | `actions.rs` | Registry of all available shortcut actions |

## Change Impact

| If you change... | Check also... |
|------------------|---------------|
| `AppSettings` fields | `default_settings.json`, `useSettings.ts`, `settingsStore.ts` |
| Tauri commands | Run `bun run tauri dev` to regenerate `bindings.ts` |
| Remote STT API format | `encode_wav_bytes()` in audio_toolkit |
| Connector message format | Handy Connector extension expects `{text, ts}` |
| Prompt templates | Variables: `${instruction}` (voice), `${output}` (selected text) |

## Platform Limitations

- **Remote STT**: Windows only (uses Windows Credential Manager for API key storage)
- **AI Replace Selection**: Windows only (uses Windows-specific selection capture via `input.rs`)
- **Connector**: Cross-platform (simple HTTP client)

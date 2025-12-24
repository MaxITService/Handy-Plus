# Handy (Fork)

A fork of [cjpais/Handy](https://github.com/cjpais/Handy) with additional features for Windows users.

## Fork Features

### 🌐 Remote STT API Support
Use any OpenAI-compatible speech-to-text API instead of local Whisper model. Useful when:
- You want faster transcription via cloud services (Groq, Deepgram, etc.)
- Your machine lacks GPU acceleration
- You prefer a hosted STT solution

### 🤖 AI Replace Selection (Windows only)
Voice-controlled text transformation:
1. Select text in any application
2. Hold the AI Replace shortcut
3. Speak your instruction (e.g., "translate to French", "fix grammar", "summarize")
4. Release — selected text is replaced with AI-processed result

Uses your configured LLM provider (OpenAI, Anthropic, etc.) with customizable prompts.

### 📤 Send to Extension (ChatGPT/Perplexity integration)
Send transcribed voice directly to AI chat interfaces via [Handy Connector](https://github.com/user/handy-connector) Chrome extension:
- **Voice to ChatGPT**: Speak and send directly to ChatGPT conversation
- **Voice + Selection**: Combine voice instruction with selected text context
- **File attachments**: Images and files support (planned)

### ⚙️ Additional Settings
- Customizable AI Replace prompts (system prompt, user template)
- "No selection" mode for AI Replace without selected text
- Remote STT debug logging

## Limitations
- **AI Replace Selection**: Windows only
- **Remote STT**: API keys stored in Windows Credential Manager

## Original Features
All original Handy features: local Whisper (Small/Medium/Turbo/Large), VAD, global shortcuts, push-to-talk, LLM post-processing, transcription history.

## License
MIT License — NO WARRANTIES.

# Hush privacy

Hush has no analytics, telemetry, application account or cloud history service. Your selected provider determines where inference runs. The application never needs your recordings or keys in a public GitHub issue.

## Recording and inference

Recording begins through your dictation shortcut or recording control. The optional prepared microphone is stopped while idle. Microphone tests stay local and are not kept as transcript history.

With **Groq** selected, dictation sends the recorded audio, language/model choice and dictionary hints to Groq using your API key. Enabled AI cleanup, Command Mode and Improve prompt send the text needed for the requested editing task. Groq's own data policies apply. With **offline** selected, Whisper runs on this computer; optional text editing uses the configured local Ollama service, without automatic cloud fallback. Offline engine/model downloads occur when requested in Settings.

Context inspection is optional. Selected-text/whole-field editing requires its explicit action and permission; protected fields are refused. Hush does not take screenshots for context. Prompt-improvement contents stay transient and are excluded from automatic history and diagnostics.

## Storage and deletion

The Groq key uses Windows Credential Manager. History/settings use the Windows user's application-data folder under `io.chahatesh.hushline`; transcript text and settings are unencrypted. Recordings saved with history use Windows user-bound DPAPI encryption. They are kept for 30 days, up to 1,000 recordings or 512 MiB, with older audio removed first. Successful and failed retries retain the recording.

Private mode or disabled recording storage keeps only the latest three recordings in memory, bounded to 64 MiB, until deletion or exit. Cancelled new recordings are discarded. Deleting a transcript removes its linked audio; Clear all removes transcripts and recordings, including failures without text. Configuration backups exclude keys, audio, history and scratchpad contents. Explicit WAV downloads are ordinary unencrypted files; manage those separately. Clearing local records cannot remove existing backups or recall data already sent to a provider.

Diagnostics hold stage timings and fixed outcome codes, without audio, transcript/prompt contents, field text or keys. Recording failure metadata uses fixed messages instead of provider payloads. Uninstallation may retain the user profile/key for reinstall; delete records and remove the key in Hush first if desired.

## Distribution services

The installer may obtain the Microsoft WebView2 system runtime when needed. Explicit project/documentation links open the associated websites. Code signing uses build binaries and GitHub metadata only; user profiles are never signing inputs. A trusted signature does not change the application's inference or storage choices.

Provider references and retention choices are also documented in [README](README.md#privacy) and [Architecture](ARCHITECTURE.md). Report security concerns using [SECURITY.md](SECURITY.md).

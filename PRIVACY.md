# Hush privacy

Hush sends no usage analytics or telemetry and has no application account or cloud history service. Your selected provider determines where inference runs. The application never needs your recordings or keys in a public GitHub issue.

## Recording and inference

Recording begins through your dictation shortcut or recording control. The optional prepared microphone is stopped while idle. Microphone tests stay local and are not kept as transcript history.

With **Groq** selected, dictation sends the recorded audio, language/model choice and dictionary hints to Groq using your API key. Enabled AI cleanup, Command Mode and Improve prompt send the text needed for the requested editing task. Groq's own data policies apply. With **offline** selected, Whisper runs on this computer; optional text editing uses the configured local Ollama service, without automatic cloud fallback. Offline engine/model downloads occur when requested in Settings.

Context inspection is optional. Selected-text/whole-field editing requires its explicit action and permission; protected fields are refused. Hush does not take screenshots for context. Prompt-improvement contents stay transient and are excluded from automatic history and diagnostics.

## Storage and deletion

The Groq key uses Windows Credential Manager. History/settings use the Windows user's application-data folder under `io.chahatesh.hushline`; transcript text and settings are unencrypted. Recordings saved with history use Windows user-bound DPAPI encryption. Transcript text has no automatic expiry and is kept until you delete it. Saved audio is kept for 30 days, up to 1,000 recordings or 512 MiB, with older audio removed first. Successful and failed retries retain the recording.

Private mode or disabled recording storage keeps only the latest three recordings in memory, bounded to 64 MiB, until deletion or exit. Cancelled new recordings are discarded. Deleting a transcript removes its linked audio; Clear all removes transcripts and recordings, including failures without text. Configuration backups exclude keys, audio, history and scratchpad contents. Explicit WAV downloads are ordinary unencrypted files; manage those separately. Clearing local records cannot remove existing backups or recall data already sent to a provider.

Diagnostics hold stage timings and fixed outcome codes, without audio, transcript/prompt contents, field text or keys. Recording failure metadata uses fixed messages instead of provider payloads. Uninstallation may retain the user profile/key for reinstall; delete records and remove the key in Hush first if desired.

## Local statistics and writing profiles

Statistics aggregate all retained transcript text and metadata locally, including dates, application names, word counts, speaking duration and processing time. Every 10,000 saved words produces a writing-profile snapshot using fixed English topic/vocabulary matching. Derived profiles contain counts and recognized vocabulary, not excerpts or raw audio. No model is downloaded or called for this feature, and no history is sent to Groq for profiles. The profile is a description of writing topics, not a biometric voiceprint or a judgment about personal traits.

Private sessions do not enter history, statistics, or profiles. Existing history remains visible after enabling private mode. Deleting records removes cached insights and rebuilds totals/profiles from the remaining records. Clear all removes saved text, recordings and derived statistics/profiles. Explicit statistics exports contain counts, dates, application names and profile evidence; text-history exports contain the complete retained text history. Manage downloaded files separately.

## Distribution services

The installer may obtain the Microsoft WebView2 system runtime when needed. Explicit project/documentation links open the associated websites. Code signing uses build binaries and GitHub metadata only; user profiles are never signing inputs. A trusted signature does not change the application's inference or storage choices.

Provider references and retention choices are also documented in [README](README.md#privacy) and [Architecture](ARCHITECTURE.md). Report security concerns using [SECURITY.md](SECURITY.md).

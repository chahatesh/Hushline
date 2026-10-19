# Changelog

## 0.2.0 — unreleased

- Fix overlapping shortcut release routing, paired key-up handling, and rapid reactivation; ordinary Space presses do not open the dashboard.
- Keep hold-to-talk recording through pauses; automatic silence stop applies to hands-free mode.
- Default to one-action paste and migrate the old typing default; retain guarded clipboard restoration and an explicit compatibility-typing option.
- Remove duplicate accessibility inspections from the insertion critical path.
- Add local automatic notes/list/paragraph layout with Literal-mode and identifier protection.
- Add checksum-pinned, one-click offline Whisper Tiny/Base packs and optional loopback-only Ollama editing. Offline mode never falls back to Groq.
- Replace the large hidden overlay with a small resting pill, bottom/side docking, 160 ms expansion, opaque active state, and voice-reactive waveform.
- Refine the dashboard and add offline, formatting, dock preview, and shortcut-recording controls.

## 0.1.0 — unreleased

- Initial Windows Rust/Tauri/Solid application, original Hushline identity and MIT source license.
- WASAPI microphone capture, energy-based voice activity detection, in-memory PCM WAV encoding, asynchronous Groq transcription with cancellation and retries.
- Modifier-only global shortcuts, hands-free recording, configurable bindings, tray lifecycle, and a nonactivating floating overlay.
- Focus/protection checks, Unicode insertion, optional guarded clipboard paste, transcript recovery, and selected-text edit previews.
- Local SQLite history, dictionary, exact-utterance snippets, writing styles, scratchpad, statistics, configuration backup, secure credential storage, and privacy controls.
- Automated checks, Windows packaging, and explicit release-verification requirements.

This version is not declared production-ready. See docs/QA.md and docs/FEATURES.md for verification evidence and known gaps. Live Groq-to-application acceptance must pass before a public release.

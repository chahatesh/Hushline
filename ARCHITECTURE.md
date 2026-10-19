# Hushline architecture

Hushline targets Windows 10/11 x64. Rust owns recording, credentials, storage, inference, safety checks, and input. SolidJS renders a local Tauri 2 WebView2 dashboard on demand. A separate nonactivating Win32/GDI window paints the small overlay without a browser renderer. Background startup creates no webviews. Closing the dashboard releases it while the app remains in the system tray; Quit terminates the process. The dashboard is not needed to complete dictation. The native overlay owns no API key, transcript, or selected text. It receives only phase, status message, activity level, and elapsed seconds.

## Critical path

```
Windows low-level keyboard/mouse hooks
  → action channel → Rust session actor
  → dedicated WASAPI capture thread (CPAL)
  → 16 kHz mono PCM WAV in memory
  → Groq /audio/transcriptions
  → conservative local normalization + exact dictionary rules
  → optional explicit Groq /chat/completions
  → validate original target → Unicode input or guarded paste
```

The hook thread has a Windows message pump and performs no microphone, disk, HTTP, or accessibility calls. A channel forwards events to the actor. Input hooks ignore injected input. The capture stream exists only while recording. A preallocated mono buffer bounds audio memory by the configured recording duration. Meter events are limited to 20 Hz during capture; there is no idle audio polling. Energy detection stops sustained silence and suppresses very quiet recordings; it is not a semantic speech classifier. A Hann-windowed sinc filter resamples device audio before 16-bit WAV encoding. Recordings are never written to disk.

## Modules

| Module | Responsibility |
| --- | --- |
| `audio.rs` | Device enumeration, capture lifecycle, metering, silence limits, resampling, encoding |
| `shortcuts.rs`, `native/hooks.rs` | Chord validation, event-driven global hooks, focus/input invalidation |
| `groq.rs` | Shared HTTP client, Groq multipart speech requests, model discovery, optional text editing, retry/cancellation |
| `text.rs` | Conservative cleanup, whole-phrase single-pass replacements, exact-trigger snippets, validation |
| `native/overlay.rs` | Native GDI overlay, nonactivation, reduced-motion-aware active timers, DPI/monitor placement |
| `native/windows.rs` | Credential Manager, UI Automation protection/identity/selection checks, Unicode delivery |
| `native/clipboard.rs` | Supported-format snapshots, paste dispatch, sequence-number guarded restoration |
| `engine.rs` | Session actor, cancellation generations, path selection, tray/windows, narrow IPC commands |
| `model.rs`, `store.rs` | Validated settings and local SQLite state, history, atomic configuration restore |
| `src/ui.tsx` | Dashboard, onboarding, editing tools, settings |
| `scripts/`, `.github/workflows/` | Validation, packaging, benchmarking, release gates |

## Insertion safety

Each recording snapshots the foreground HWND, UI Automation runtime ID, selected-range geometry, and an input/focus generation. Command Mode additionally reads only the selected text after opt-in. Completion rechecks the same identity, geometry, protection, and generation. Input or focus changes preserve the transcript for manual recovery. A Command Mode approval may explicitly bring the original window forward; its exact selection and geometry must still match. No selection is blindly restored. A final validation occurs immediately before dispatch; Windows does not offer an atomic cross-process focus-and-insert transaction, so a small remaining race is documented.

Unicode input avoids clipboard mutation. Optional paste mode snapshots global-memory formats and refuses unsupported formats before clearing. Clipboard sequence numbers prevent restoration over newer copied content. Dispatch is not a universal delivery receipt. Automatic terminal insertion is intentionally blocked to prevent pasted newlines from executing commands.

## Privacy and network boundaries

The key enters a password field once and goes directly to native Credential Manager code. Snapshots return only a boolean indicating a saved key. Cloud requests occur only for model verification, dictation, or enabled text processing. Fast mode has no language-model request. Nearby text, screenshots, browser history, and window titles are not collected. Context style matching uses executable names only. Command Mode sends the user's selection and instruction. No updater, telemetry, or cloud history service exists.

SQLite uses WAL, secure deletion, parameterized statements, and a busy timeout. Backup restore validates all fields before one transaction. Configuration backup excludes API keys, history, and scratchpad. The database is not encrypted. Private mode suppresses new transcript rows but does not delete existing ones. History currently loads/exports the latest 1,000 records; statistics are explicitly scoped to them.

## Limits and future engineering

Read docs/FEATURES.md for gaps. Long recordings are bounded to ten minutes rather than chunked; duplicate microphone display names are not yet disambiguated by persistent device ID. Browser-domain matching, arbitrary native rich-text insertion, robust semantic repair without AI, a signed updater, and broad application compatibility need additional work. A browser preview can review local UI collections only; it explicitly disables and does not simulate native behavior.

## 0.2 changes

`offline.rs` owns an opt-in verified speech pack, cancellable hidden whisper.cpp child processes and optional local Ollama editing. The cloud backend is selected explicitly; no inference error changes providers. Raw WAV goes through stdin; transcript JSON has a temporary-directory guard. Models and engine resources remain outside SQLite and configuration backups. The speech child loads on demand and exits after each recording, so the resting app does not retain model RAM.

The hook router starts actions only on key-down edges, retains paired consumed key-up state, and never falls back to push-to-talk on a more specific chord's release. Held sessions carry a stop request so a new rapid press can replace a stopping capture. A shared atomic controls silence stopping for hands-free sessions; hold-to-talk ignores silence. Shortcut recording suspends actions with a 15-second native expiry.

Clipboard paste is the default. Its final target check replaces two redundant UIA traversals that previously occurred in `insert`. The clipboard is still snapshotted before mutation and restored only while its sequence matches. Compatibility Unicode injection is an explicit single-line option, not an automatic fallback.

The GDI overlay has an idle resting pill and bottom/left/right geometry. Only active processing and the 160-ms size/opacity transition use timers; microphone meter events repaint capture. Idle has no timer. It uses a double-buffered surface, a rounded window region, full active alpha, and `WS_EX_NOACTIVATE`. Dock/monitor changes snap rather than flying across the screen. Dashboard and native bar share settings but do not share a webview.

`text::format` performs local structural whitespace/list transformations after normalization. It keeps existing explicit newlines and bypasses Literal/non-English transformations. Technical spans are excluded from sentence splitting. Optional AI cleanup remains a separate opt-in path.

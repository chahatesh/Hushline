# Hush architecture

Hush targets Windows 10/11 x64. Rust owns recording, credentials, storage, inference, safety checks, and input. SolidJS renders a local Tauri 2 WebView2 dashboard on demand. A separate nonactivating Win32/GDI window paints the small overlay without a browser renderer. Background startup creates no webviews. Closing the dashboard releases it while the app remains in the system tray; Quit terminates the process. The dashboard is not needed to complete dictation. The native overlay owns no API key, transcript, or selected text. It receives only phase/stage, status message, activity level, and elapsed seconds. Versioned sections below retain historical decisions; the 0.4.0 contract supersedes older timer placement and automatic-cleanup rules.

## Critical path

```
Windows low-level keyboard/mouse hooks
  → action channel → Rust session actor
  → cancellable startup pause → dedicated native WASAPI owner thread
  → 16 kHz mono PCM WAV in memory
  → Groq /audio/transcriptions
  → corrected-meaning local cleanup + exact dictionary rules + writing style
  → optional explicit Groq /chat/completions
  → validate original target → Unicode input or guarded paste
```

The hook thread has a Windows message pump and performs no microphone, disk, HTTP, or accessibility calls. A channel forwards events to the actor. Input hooks ignore injected input. With fast startup enabled, the audio thread prepares a **stopped** WASAPI client asynchronously when the app opens. It calls Start only for an explicit recording and Stop/Reset between sessions. No samples are read while idle; its command receiver blocks without a timer. Disabling fast startup releases the prepared client. CPAL is retained only for microphone-name enumeration.

A fallibly preallocated mono buffer exists only during a recording/encoding job and bounds memory by the configured duration. Available packets are drained every 20 ms while recording; native meter events target 50 Hz and dashboard IPC is limited to 10 Hz. Default-device identity is checked on activation, packet discontinuity and missing data are surfaced, and the last buffered frames are drained after Stop. Encoding runs separately from endpoint ownership so a new press does not wait for the previous resampling job. Energy-based silence stopping is optional and off by default; it never stops hold-to-talk. The ten-minute default/max is explicit. A Hann-windowed sinc filter resamples device audio before 16-bit WAV encoding. Successful/cancelled recordings are not persisted. Failed recordings can be stored as per-user DPAPI-encrypted blobs in SQLite; private captures use memory-only recovery.

## Modules

| Module | Responsibility |
| --- | --- |
| `audio.rs`, `audio/wasapi.rs` | Stopped-client preparation, native Start/Stop/Reset, device identity, packet reading, metering, limits and encoding |
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

Each recording captures the original foreground HWND and input/focus generation, then starts bounded UI Automation inspection for field identity and selected-range geometry. A pending inspection is retained by the processing job after recording release and resolved alongside transcription, not replaced with a failed target just because capture stopped. Command Mode additionally reads only the selected text after opt-in. Completion rechecks the same identity, geometry, protection, and generation. Input or focus changes preserve the transcript for manual recovery. A Command Mode approval may explicitly bring the original window forward; its exact selection and geometry must still match. No selection is blindly restored. A final validation occurs immediately before dispatch; Windows does not offer an atomic cross-process focus-and-insert transaction, so a small remaining race is documented.

Unicode input avoids clipboard mutation. Default paste mode snapshots global-memory formats and refuses unsupported formats before clearing. An owned worker acknowledges after guarded dispatch, then retains its clipboard owner for the conservative 600-ms restoration delay. The completion UI no longer waits for that delay. Cleanup notices travel separately to the actor. Clipboard sequence numbers prevent restoration over newer copied content. A two-second delivery deadline cancels a late paste attempt. Slow UI Automation checks run before clipboard ownership/mutation, and the cancellation token is checked immediately before dispatch. Dispatch is not a universal delivery receipt. Automatic terminal insertion is intentionally blocked to prevent pasted newlines from executing commands.

## Privacy and network boundaries

The key enters a password field once and goes directly to native Credential Manager code. Snapshots return only a boolean indicating a saved key. Cloud requests occur only for model verification, dictation, or enabled text processing. Fast mode has no language-model request. Ordinary dictation does not collect nearby text, screenshots, browser history, or window titles. Context style matching uses executable names only. Explicit Command Mode reads the authorized selection and sends it with the instruction. Explicit Improve prompt reads only the whole focused editable field under its bounded input policy; it never reads unrelated fields or surrounding document text. No updater, telemetry, or cloud history service exists.

SQLite uses WAL, secure deletion, parameterized statements, and a busy timeout. Backup restore validates all fields before one transaction. Configuration backup excludes API keys, history, recovery audio, scratchpad, transient prompt previews, and diagnostics. The database is not encrypted. Private mode suppresses new transcript rows but does not delete existing ones. History currently loads/exports the latest 1,000 records; statistics are explicitly scoped to them.

## Limits and future engineering

Read docs/FEATURES.md for gaps. Long recordings are bounded to ten minutes rather than chunked; duplicate microphone display names are not yet disambiguated by persistent device ID. Browser-domain matching, arbitrary native rich-text insertion, robust semantic repair without AI, a signed updater, and broad application compatibility need additional work. A browser preview can review local UI collections only; it explicitly disables and does not simulate native behavior.

## 0.2 changes

`offline.rs` owns an opt-in verified speech pack, cancellable hidden whisper.cpp child processes and optional local Ollama editing. The cloud backend is selected explicitly; no inference error changes providers. Raw WAV goes through stdin; transcript JSON has a temporary-directory guard. Models and engine resources remain outside SQLite and configuration backups. The speech child loads on demand and exits after each recording, so the resting app does not retain model RAM.

The hook router starts actions only on key-down edges, retains paired consumed key-up state, and never falls back to push-to-talk on a more specific chord's release. Held sessions carry a stop request so a new rapid press can replace a stopping capture. A shared atomic controls silence stopping for hands-free sessions; hold-to-talk ignores silence. Shortcut recording suspends actions with a 15-second native expiry.

Clipboard paste is the default. Its final target check replaces two redundant UIA traversals that previously occurred in `insert`. The clipboard is still snapshotted before mutation and restored only while its sequence matches. Compatibility Unicode injection is an explicit single-line option, not an automatic fallback.

The GDI overlay has an idle resting pill and bottom/left/right geometry. Only active processing and the 160-ms size/opacity transition use timers; microphone meter events repaint capture. Idle has no timer. It uses a double-buffered surface, a rounded window region, full active alpha, and `WS_EX_NOACTIVATE`. Dock/monitor changes snap rather than flying across the screen. Dashboard and native bar share settings but do not share a webview.

`text::format` performs local structural whitespace/list transformations after normalization. It keeps existing explicit newlines and bypasses Literal/non-English transformations. Technical spans are excluded from sentence splitting. Optional AI cleanup remains a separate opt-in path.

## 0.2.1 changes

The native bar is a 48×16 logical-pixel resting capsule (16×48 at the side), expanding to 136×30 at the bottom or 30×112 at the side. No microphone icon is drawn. A signed-distance coverage rasterizer supplies premultiplied per-pixel alpha to UpdateLayeredWindow, preserving antialiased outer pixels that a binary window region clipped. The waveform, cancel target and accessible window title remain; no idle rendering timer was added.

Local formatting handles natural list introductions and repeated restart fragments, and processes long prose within explicit line breaks rather than bypassing the whole transcript. Automatic AI cleanup rejects missing identifiers/numbers or substantial shortening; this is a conservative guard, not semantic proof. Command Mode retains its explicit authority to shorten selected text. GPT-OSS editing requests use Groq's documented low reasoning effort; unrelated configurable models are not sent that field. Local Ollama edits disable thinking and reserve an explicit context/output budget, rejecting oversized inputs instead of relying on a truncating default. Models still unload after editing. See the small-model trial in docs/LOCAL_MODEL.md.
## 0.3.0 polishing and error presentation

Settings schema 4 adds `polishPolicy` (`always`, `sometimes`, `never`). Legacy `enhanced`/Polished opt-ins migrate to Always; other saved profiles remain Never. Never overrides automatic per-app AI requests. Literal mode and exact snippet expansion keep their explicit contracts, and Command Mode remains separate. Sometimes uses deterministic length/cue/style checks; it does not call a model to decide whether to call a model. Groq remains the only cloud provider. Offline speech routes optional editing only to the local Ollama service.

Automatic AI edits retain the existing content/identifier/number guard and local fallback. The actor receives a generation-tagged formatting status so cancelled or superseded work cannot update the current overlay. Local postprocessing handles clear list structures, paragraphs and terminal fragment punctuation without adding sentence dots unconditionally.

The native overlay extends its transparent surface upward for a measured, wrapped error/recovery card. The pill retains its own geometry and cancel hit area. The card participates in per-pixel alpha rendering, respects the monitor work area and uses `WS_EX_NOACTIVATE`; it owns no transcript or key. It introduces no idle timer or separate browser renderer.

The visible product name is Hush. Stable internal names (`io.chahatesh.hushline`, `hushline.sqlite`, `Hushline/Groq`, autostart `Hushline`, `hushline.exe`) intentionally remain unchanged for upgrades.

## 0.3.1 responsive unified surface

The bottom bar grows from 48×16 to 168×48 logical pixels, keeping its lower edge fixed. Side bars grow from 16×48 to 40×128 with the same lower anchor. Errors and recovery notices extend the same rounded body upward. Opening takes 170 ms (220 ms for notices), closing 130 ms; Windows reduced-motion settings bypass the transition. Signed-distance alpha and interpolated corners keep edges smooth. Processing refreshes at 25 Hz; listening follows microphone readings. No idle timer was added.

A thread-owned GDI DIB/DC and pixel buffer are reused at a stable size, with allocation only when dimensions change. Window titles update only on content changes. An atomic pending flag coalesces native update messages so rendering receives the latest level without queuing every reading. The bar remains independent of the dashboard and does not activate the target window.

Dictionary prompts retain distinct complete terms within an 800-character application budget; oversized rules are skipped so later useful vocabulary remains eligible. Groq's supported prompt/token limits still apply. This is a hint-quality improvement, not a measured general speech-accuracy claim.

WASAPI device positions and discontinuity flags identify gaps after the first packet. The recorder preserves later packets and inserts bounded silence for a known missing interval, avoiding a false join between words. A generation-tagged warning is displayed during capture. Any gap invalidates automatic insertion and disables snippets/automatic AI editing for that recording; Command Mode fails before transcription or replacement. Normal dictation can transcribe the available audio for manual review/copy. Genuine read/device errors still fail. The missing original speech cannot be recovered, and the driver/scheduling cause is not claimed fixed.

## 0.3.2 capture policy and compact presentation

The 0.3.1 any-gap review gate was too strict for real microphone startup. Timing-only diagnostics reproduced a 5,887-frame discontinuity at 48 kHz (about 123 ms), followed by normal capture. Review is now based on measured missing duration: at least 250 ms in one gap or 500 ms accumulated. Brief/untimed driver discontinuity flags alone do not disable paste, snippets or optional polishing. This is an explicit usability policy, not a claim that short gaps lose no words. Disconnected/stalled devices still fail, known missing intervals remain bounded silence, and substantial loss still invalidates insertion and rejects Command Mode.

The overlay tracks session generation and notice identity separately from meter readings. A single 2.5-second timer contracts expanded details to a 168×28 bottom or 40×112 side bar. The same lower dock edge stays anchored; recording and processing continue. New sessions/notices reset a deadline, so a queued timer from an earlier session cannot immediately collapse a new one. Meter updates never rearm this timer. Active bars show the waveform, timer and close control without a listening label. Bottom bars initially measure 168×32; the waveform stays centered and the time sits beside × on the right. Side bars put the timer and × below the waveform. The close control cancels even after contraction. Listening clicks stop hands-free capture, processing clicks cancel, and right-click opens the dashboard. Completion/errors return to idle within 2.5 seconds; recovery text remains in the dashboard. Idle has no timer. A class arrow cursor and WM_SETCURSOR handling prevent a stale busy cursor over the native surface.

## Processing deadline and recoverable audio

The speech stage runs under a cancellable deadline (`deadline.rs`), default five seconds, configurable 3–120. It covers the provider request, response parsing, retries and offline process. A child token is cancelled and the future dropped on expiry; a late result cannot reach delivery. Offline children use kill-on-drop. The actor publishes the exact timeout error immediately and moves recovery encryption/SQLite work to a blocking worker. Provider failures also retain audio; user cancellation does not. Optional text cleanup has a separate five-second limit with local-text fallback; selected-text command editing has a ten-second limit and never applies a failed result.

`recovery.rs` uses user-bound Windows DPAPI with a fixed application entropy value and a payload digest. Only encrypted audio blobs are written. Metadata is limited to ID, timestamp, duration, review flag and storage kind. Normal settings/history remain unencrypted. Recovery is limited to three clips, combining disk and memory, with a seven-day expiry checked on startup/access/save. Private mode at recording or failure prevents disk persistence. A protection/storage failure falls back to memory with an explicit warning. Stored encrypted clips are removed after successful retry or explicit deletion; expiry cleanup is event-driven. Configuration exports exclude audio. SQLite secure deletion and WAL checkpointing are used; this is not forensic erasure of storage backups.

A retry creates a new generation and cancellation token and has no insertion target or retained selection. It uses current provider settings; snippets can expand but cannot offer an authorized selection replacement. Text is shown for review/copy. Failed retries leave the original clip intact without duplicating it. No background retry/upload loop or additional cloud provider is introduced.

## 0.4.0 recording state and deadlines

The actor has a `starting` phase for the configurable 300 ms arm delay (`startupDelayMs`, 0–1,500) and microphone startup. Releasing a held shortcut before samples arrive abandons that capture; a queued startup completion cannot revive it. Double-tap arbitration retains its existing preference and separate 350 ms release window. Recording-ready feedback begins only on first frames: one optional original cue (`recordingCueEnabled`) and the bright live waveform. Idle, preparation, and processing stay visually distinct; preparing/transcribing/polishing/pasting use a dark bar. The timer is left of the centered waveform and × is right; side docks put time above and × below.

Stages exposed through the view are `arming`, `microphone`, `recording`, `preparing_audio`, `transcribing`, `polishing`, `inserting`, and `prompt_editing`. Microphone startup is bounded to five seconds, speech recognition uses the existing configurable five-second default, optional cleanup has a five-second local fallback, selected-text command editing has ten seconds, and delivery has two seconds. Deadlines/cancellation are generation-scoped. Opening the dashboard or copying text during processing can invalidate insertion authority without abandoning transcription or leaving a permanent processing state. Late work cannot update a superseding session, and cancellation is rechecked before native dispatch.

Target inspection stays outside the audio worker and input hook. Its pending result travels with the original recording job under a bounded 1.2-second native-work wait. Focus/input changes never authorize a new destination implicitly. Native worker timeouts preserve text and revoke late insertion; they do not claim to forcibly terminate a blocked Windows accessibility provider. Existing clipboard-format limits, sequence guards, protected-field checks, and terminal refusal remain in force.

`Diagnostics` retains at most 200 timing/outcome events across up to 50 session generations in memory. Its `StageDiagnostic` contract contains only `generation`, `stage`, `elapsedMs`, and `outcome`. Snapshot/export never contains transcript or prompt content, raw audio, field text, or an API key. Export is explicit from Privacy & data; quitting clears diagnostics. This is operational evidence, not telemetry or a persistent transcript log.

## 0.4.0 writing and AI contract

Settings schema 5 separates `writingStyle` (`casual`, `standard`, `formal`) from `polishPolicy` (`always`, `sometimes`, `never`). Casual keeps normal capitalization without added periods or automatic layout; Standard applies sentence punctuation and clear paragraph/list structures; Formal adds professional grammar/concise wording when AI is enabled. Explicit spoken formatting is honored in each style. The separate `mode: literal` advanced option preserves recognition output and bypasses cleanup/style/AI. Legacy Clean migrates to Standard, Polished to Formal with internal Clean mode, and Literal keeps Literal with Casual selected. Existing provider, double-tap, shortcuts, credentials, history, and timeout settings survive migration. Earlier schema-4 automatic-AI opt-ins retain their established migration semantics.

Local cleanup resolves only explicit, supported corrections and retains ambiguous wording. Its corrected-meaning record identifies retracted statements, allowing an obsolete value such as “three” to disappear from “I have three dogs. Wait, no, I actually have four” while protecting unrelated content. Automatic AI instructions match the writing style and ask for only the edited text, with no invented facts or requirements. Validation compares against the corrected meaning, guards technical spans and unrelated facts, and falls back to local text on rejection or timeout. This remains a heuristic boundary, not a semantic-equivalence proof. List parsing handles explicit introductions, numbered/bullet cues, multi-sentence items, and mixed prose conservatively; plain conjunctions do not establish a list.

## 0.4.0 explicitly approved whole-field editing

Improve prompt is initially invoked only by its configurable `shortcuts.improvePrompt` (default Ctrl+Win+P), without microphone activation. `promptImproveMode` defaults to `clarify`; `expand` permits additions only under labeled optional suggestions/assumptions. Both use the configured Command Mode model or the configured local Ollama model in offline mode, under a cancellable 20-second inference deadline. There is no cloud fallback from offline mode.

The original field snapshot is transient. Input must be one supported, nonempty editable field and at most 32,000 UTF-8 bytes; protected/read-only inputs, terminals, oversized fields, unsupported controls, and providers exposing unrelated document content are rejected without truncation. Approved whole-field replacement uses the supported native edit/accessibility operation in one action. This direct operation is limited to Improve prompt; ordinary dictation retains its existing insertion engine.

The view includes `previewKind`, `previewId`, `original`, and `preview`. `previewKind: "prompt"` distinguishes a whole-field edit from selected-text/snippet previews. Proposed output is nonempty and at most 100,000 UTF-8 bytes. The dashboard edits that proposal locally and calls `apply_preview({previewId,text})`; identity, the complete original field text, protection, and cancellation are revalidated before replacement. Stale preview IDs or changed targets cannot apply. `regenerate_prompt({mode})` works from the retained original snapshot and remembers Clarify/Expand. Apply, Copy, and Cancel remain explicit; a failed replacement retains the proposal for copying. Prompt source/output are excluded from automatic history, configuration backup, diagnostics, and recovery-audio metadata. Windows application support and end-to-end acceptance remain separately tracked in QA/FEATURES.

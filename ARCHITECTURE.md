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
  → validate original target → native edit or guarded paste (optional Unicode compatibility)
```

The hook thread has a Windows message pump and performs no microphone, disk, HTTP, or accessibility calls. A channel forwards events to the actor. Input hooks ignore injected input. With fast startup enabled, the audio thread prepares a **stopped** WASAPI client asynchronously when the app opens. It calls Start only for an explicit recording and Stop/Reset between sessions. No samples are read while idle; its command receiver blocks without a timer. Disabling fast startup releases the prepared client. CPAL is retained only for microphone-name enumeration.

A fallibly preallocated mono buffer exists only during a recording/encoding job and bounds memory by the configured duration. Available packets are drained every 20 ms while recording; native meter events target 50 Hz and dashboard IPC is limited to 10 Hz. Default-device identity is checked on activation, packet discontinuity and missing data are surfaced, and the last buffered frames are drained after Stop. Encoding runs separately from endpoint ownership so a new press does not wait for the previous resampling job. Energy-based silence stopping is optional and off by default; it never stops hold-to-talk. The ten-minute default/max is explicit. A Hann-windowed sinc filter resamples device audio before 16-bit WAV encoding. Completed recordings are protected before processing and retained as per-user DPAPI-encrypted blobs when recording storage/history are enabled; private captures use bounded memory recovery. Cancelled new recordings are discarded.

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
| `native/edit.rs` | One undoable insertion into documented Unicode Windows edit controls, with selection and cancellation fences |
| `engine.rs` | Session actor, cancellation generations, path selection, tray/windows, narrow IPC commands |
| `model.rs`, `store.rs` | Validated settings and local SQLite state, history, atomic configuration restore |
| `src/ui.tsx` | Dashboard, onboarding, editing tools, settings |
| `scripts/`, `.github/workflows/` | Validation, packaging, benchmarking, release gates |

## Insertion safety

Each recording captures the original foreground HWND and input/focus generation, then starts bounded UI Automation inspection for field identity and selected-range geometry. A pending inspection is retained by the processing job after recording release and resolved alongside transcription, not replaced with a failed target just because capture stopped. Command Mode additionally reads only the selected text after opt-in. Completion rechecks the same identity, geometry, protection, and generation. Input or focus changes preserve the transcript for manual recovery. A Command Mode approval may explicitly bring the original window forward; its exact selection and geometry must still match. No selection is blindly restored. A final validation occurs immediately before dispatch; Windows does not offer an atomic cross-process focus-and-insert transaction, so a small remaining race is documented.

Unicode input avoids clipboard mutation. Default smart paste uses one undoable EM_REPLACESEL for documented foreign-process Unicode Edit/RichEdit controls. UIA identity/protection, native selection indices/text length, input generation and cancellation are checked before dispatch. Visual reflow alone does not invalidate native selection. Same-process controls, custom controls and browsers retain clipboard delivery. A timed-out synchronous edit may have been accepted; it is never automatically retried or followed by another insertion method. Clipboard fallback snapshots supported global-memory formats, native bitmaps and enhanced metafiles and refuses unsupported formats before clearing. Snapshots are bounded to 32 MiB per format and 96 MiB total; palettes, classic metafiles and unknown private handles are unsupported. An owned worker acknowledges after guarded dispatch, then retains its clipboard owner for the conservative 600-ms restoration delay. The completion UI no longer waits for that delay. Cleanup notices travel separately to the actor. Clipboard sequence numbers prevent restoration over newer copied content. A two-second delivery deadline cancels a late paste attempt. Slow UI Automation checks run before clipboard ownership/mutation, and the cancellation token is checked immediately before dispatch. Dispatch is not a universal delivery receipt. Automatic terminal insertion is intentionally blocked to prevent pasted newlines from executing commands.

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

Local formatting handles natural list introductions and repeated restart fragments, and processes long prose within explicit line breaks rather than bypassing the whole transcript. Automatic AI cleanup rejects missing identifiers/numbers or substantial shortening; this is a conservative guard, not semantic proof. Command Mode retains its explicit authority to shorten selected text. GPT-OSS editing requests use Groq's documented low reasoning effort; unrelated configurable models are not sent that field. Local Ollama edits disable thinking and reserve an explicit context/output budget, rejecting oversized inputs instead of relying on a truncating default. Models still unload after editing. See the local-model observations in docs/PERFORMANCE.md.
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

Audio-loss diagnostics use fixed timing metadata. Brief discontinuity flags alone do not disable ordinary dictation. Substantial gaps are flagged for review; ordinary dictation delivers the available speech with a notice, while interrupted Command Mode is refused. Lost audio cannot be reconstructed. Disconnected or stalled devices end capture with recoverable audio where available.

The overlay tracks generation and notice identity separately from meter readings. A 2.5-second deadline contracts recording details to a 168×28 bottom or 40×112 side bar without stopping capture or processing. New sessions/notices renew that deadline; meter readings do not. The timer sits left of the centered waveform and × sits right; side docks put the timer above the waveform and × below it. Listening clicks stop hands-free capture, processing clicks cancel, and right-click opens native quick controls. Errors and recovery notices remain readable for twelve seconds. A one-shot snooze timer can hide the resting widget for an hour, while active feedback remains visible. Idle has no animation loop. A class arrow cursor and WM_SETCURSOR handling prevent stale busy cursors.

## Processing deadline and recoverable audio

The speech stage runs under a cancellable deadline (`deadline.rs`), with a 30-second default and configurable 3–120 second short-clip wait. Longer clips get max(configured, 30 + duration_seconds/10), capped at 120 seconds; manual retries get at least 60 seconds. Fast results return immediately. Upgrading settings v5 to v6 replaces only the former five-second default; all other waits remain unchanged. The deadline covers connection, upload, response parsing, retries and the offline process. A child token is cancelled and the future dropped on expiry; late results cannot reach insertion. Offline children use kill-on-drop. Optional AI cleanup keeps its separate five-second local fallback.

`recovery.rs` protects audio using user-bound Windows DPAPI, stable application entropy and a payload digest. A completed dictation is protected on a blocking worker before key lookup or provider processing. Successful and failed audio is retained, with fixed outcome messages, attempt count and an optional transcript link in metadata. No provider payload, API key, prompt or surrounding field text is added. Storage is bounded to 30 days, 1,000 clips and 512 MiB using an atomic SQLite insert/prune transaction. Private mode or disabled recording storage uses only the latest three clips in memory, capped at 64 MiB; memory buffers are zeroed on deletion. Protection/storage failure also falls back to memory with an explicit notice. Cancelled new sessions are discarded. Normal settings/history remain unencrypted. Configuration exports exclude audio. Explicit WAV download is unencrypted. Secure deletion/checkpointing does not erase filesystem backups.

A retry creates a new generation and cancellation token without insertion authority. It loads and verifies the existing audio, uses current speech/writing settings, and retains the original recording after success, failure or cancellation. Its transcript ID is stable: successful retry updates one history result, preserving its original application and timestamp. It never automatically edits an old selection. Deleting a transcript removes its linked audio; clearing history removes all disk and memory audio, including unlinked failures. Metadata updates never reinsert deleted recordings. Listen/Download reads decrypted WAV only through explicit binary IPC; snapshots contain metadata only. The dashboard owns at most one playback URL and revokes it on replacement/deletion/close. No background retry/upload loop is introduced.

## 0.4.0 recording state and deadlines

The actor has a `starting` phase for the configurable 300 ms arm delay (`startupDelayMs`, 0–1,500) and microphone startup. Releasing a held shortcut before samples arrive abandons that capture; a queued startup completion cannot revive it. Double-tap arbitration retains its existing preference and separate 350 ms release window. Recording-ready feedback begins only on first frames: one optional original cue (`recordingCueEnabled`) and the bright live waveform. Idle, preparation, and processing stay visually distinct; preparing/transcribing/polishing/pasting use a dark bar. The timer is left of the centered waveform and × is right; side docks put time above and × below.

Stages exposed through the view are `arming`, `microphone`, `recording`, `preparing_audio`, `transcribing`, `polishing`, `inserting`, and `prompt_editing`. Microphone startup is bounded to five seconds, speech recognition uses the configurable 30-second default (with longer bounds for long clips and manual retries), optional cleanup has a five-second local fallback, selected-text command editing has ten seconds, and delivery has two seconds. Deadlines/cancellation are generation-scoped. Opening the dashboard or copying text during processing can invalidate insertion authority without abandoning transcription or leaving a permanent processing state. Late work cannot update a superseding session, and cancellation is rechecked before native dispatch.

Target inspection stays outside the audio worker and input hook. Its pending result travels with the original recording job under a bounded 1.2-second native-work wait. Focus/input changes never authorize a new destination implicitly. Native worker timeouts preserve text and revoke late insertion; they do not claim to forcibly terminate a blocked Windows accessibility provider. Existing clipboard-format limits, sequence guards, protected-field checks, and terminal refusal remain in force.

`Diagnostics` retains at most 200 timing/outcome events across up to 50 session generations in memory. Its `StageDiagnostic` contract contains only `generation`, `stage`, `elapsedMs`, and `outcome`. The diagnostic list and its export never contain transcript or prompt content, raw audio, field text, or an API key. Export is explicit from Privacy & data; quitting clears diagnostics. This is operational evidence, not telemetry or a persistent transcript log.

## 0.4.0 writing and AI contract

Settings schema 5 separates `writingStyle` (`casual`, `standard`, `formal`) from `polishPolicy` (`always`, `sometimes`, `never`). Casual keeps normal capitalization without added periods or automatic layout; Standard applies sentence punctuation and clear paragraph/list structures; Formal adds professional grammar/concise wording when AI is enabled. Explicit spoken formatting is honored in each style. The separate `mode: literal` advanced option preserves recognition output and bypasses cleanup/style/AI. Legacy Clean migrates to Standard, Polished to Formal with internal Clean mode, and Literal keeps Literal with Casual selected. Existing provider, double-tap, shortcuts, credentials, history, and timeout settings survive migration. Earlier schema-4 automatic-AI opt-ins retain their established migration semantics.

Local cleanup resolves only explicit, supported corrections and retains ambiguous wording. Its corrected-meaning record identifies retracted statements, allowing an obsolete value such as “three” to disappear from “I have three dogs. Wait, no, I actually have four” while protecting unrelated content. Automatic AI instructions match the writing style and ask for only the edited text, with no invented facts or requirements. Validation compares against the corrected meaning, guards technical spans and unrelated facts, and falls back to local text on rejection or timeout. This remains a heuristic boundary, not a semantic-equivalence proof. List parsing handles explicit introductions, numbered/bullet cues, multi-sentence items, and mixed prose conservatively; plain conjunctions do not establish a list.

## 0.4.0 explicitly approved whole-field editing

Improve prompt is initially invoked only by its configurable `shortcuts.improvePrompt` (default Ctrl+Shift+P), without microphone activation. `promptImproveMode` defaults to `clarify`; `expand` permits additions only under labeled optional suggestions/assumptions. Both use the configured Command Mode model or the configured local Ollama model in offline mode, under a cancellable 20-second inference deadline. There is no cloud fallback from offline mode.

The original field snapshot is transient. Input must be one supported, nonempty editable field and at most 32,000 UTF-8 bytes; protected/read-only inputs, terminals, oversized fields, unsupported controls, and providers exposing unrelated document content are rejected without truncation. Approved whole-field replacement uses the supported native edit/accessibility operation in one action. Whole-field replacement remains separate from ordinary dictation, which may insert at the original selection through the bounded native edit path.

The view includes `previewKind`, `previewId`, `original`, and `preview`. `previewKind: "prompt"` distinguishes a whole-field edit from selected-text/snippet previews. Proposed output is nonempty and at most 100,000 UTF-8 bytes. The dashboard edits that proposal locally and calls `apply_preview({previewId,text})`; identity, the complete original field text, protection, and cancellation are revalidated before replacement. Stale preview IDs or changed targets cannot apply. `regenerate_prompt({mode})` works from the retained original snapshot and remembers Clarify/Expand. Apply, Copy, and Cancel remain explicit; a failed replacement retains the proposal for copying. Prompt source/output are excluded from automatic history, configuration backup, diagnostics, and recovery-audio metadata. Windows application support and end-to-end acceptance remain separately tracked in QA/FEATURES.

## 0.5.0 recovery feedback

The ephemeral `recoverable` view flag exposes explicit Paste here / Copy text buttons in native recovery cards. These dispatch existing manual actions to the actor without activating the overlay. They never apply a pending preview or start another recording. Last text stays recoverable after a failed delivery. Manual operations carry a job generation and feedback operation ID; stale completion cannot overwrite a new attempt or recording. Visibility tickets invalidate old hide timers when newer feedback appears. Both gates are bounded scalar state, with no idle polling. Abandoned preparation returns quietly to idle. Diagnostics still contain only timings and fixed outcomes, not text or clipboard content.

## 0.5.1 delivery policy

Ordinary dictation delivers the available speech after an audio discontinuity, retaining a gap warning; snippets and optional AI are bypassed on interrupted audio. Interrupted editing commands remain rejected before cloud transcription. Failed/rejected automatic cleanup keeps local text, records only `local_fallback`, and does not show a formatting-error card or prevent delivery. Windows accessibility focus notifications are treated as provider hints rather than physical input: the final field/selection comparison resolves them. Foreground transitions and actual keyboard/mouse activity still invalidate the original target.

Target inspection is bounded to 2.5 seconds and delivery to 5 seconds. Clipboard workers acknowledge within 4.5 seconds, allowing up to 500 ms for clipboard availability. No mutation is retried after an uncertain dispatch. Native error/recovery/warning notices stay readable for 12 seconds; normal recording details keep their 2.5-second contraction. The actor visibility ticket and native notice identity keep old timers from dismissing newer feedback. No preference migration or idle polling is added.

## 0.5.2 field identity through layout changes

For ordinary dictation into a non-native editable field, screen coordinates no longer act as a logical caret bookmark when UIA proves both selections are collapsed. The target retains `selection_empty: Option<bool>`, obtained from [CompareEndpoints](https://learn.microsoft.com/en-us/windows/win32/api/uiautomationclient/nf-uiautomationclient-iuiautomationtextrange-compareendpoints) on the range itself. This does not retrieve selected or surrounding text. Matching HWND/runtime ID, collapsed-selection state, protection and the real input epoch still apply. Unknown/noncollapsed selections and approved previews retain rectangle checks; native edit controls keep exact selection indices and length. A field can therefore grow or move during recording without discarding its paste authority. App-managed logical caret movement within that same collapsed field is accepted; physical keyboard/mouse changes revoke delivery. No background polling, text collection or automatic repeat dispatch is added.

## 0.5.3 ordinary dictation authority

`FocusAnchor` contains only HWND, PID and real-input generation. The actor carries it through Session → Job → Completed independently of the optional early UIA receiver. Normal dictation skips early UIA unless application context is enabled; context has a separate one-slot semaphore, while critical native delivery retains two slots. Ordinary processing polls available context without waiting. Process-only app attribution remains available for history.

At delivery the clipboard worker freshly checks the focused field's ownership, enabled/password/read-only/editable and terminal properties. Provider runtime IDs are optional for this route; app-managed focus/selection changes within the anchored window are accepted. Actual clicking/typing/window changes and cancellation revoke it. Native edit controls use one undoable edit with fresh selection metadata; other fields receive one guarded clipboard paste. Read-only inspection may retry up to three times, 50 ms apart, before clipboard ownership. No mutation or uncertain dispatch is retried. Held modifiers get up to three seconds within the five-second delivery deadline. Scrolling no longer changes the input generation.

Command Mode and approved previews still use the strict original field/selection route. A confirming snippet can capture a strict target on demand; recovered recordings have no delivery anchor and stay review-only. Optional `deliveryFailure` in existing history JSON contains only a fixed outcome code; old records remain compatible and private captures do not create persistent diagnostic rows. In-memory stage diagnostics still exclude contents and keys.

Repeated recording actions during a stopping capture or processing stage are guarded before the actor can call reset_job or cancel_capture. They keep the original generation, cancellation scope and transcript alive. Escape still cancels explicitly; listening/hands-free stop and preview behavior remain unchanged.

## 0.6.0 provider reliability

Successful HTTP headers no longer end the retry scope: body reads and JSON decoding use the same three-attempt budget as recoverable HTTP/network errors. Bodies are bounded to 2 MiB and every body read/backoff observes cancellation. Short Retry-After values are respected; long rate limits and authentication/permission errors return immediately for manual recovery. HTTP 408/429/5xx can retry; no response payload is logged or persisted. An endpoint failure retains already-captured usable audio with a review flag instead of dropping it. These rules do not reconstruct missing samples or imply universal speech accuracy.

Groq contract references: [speech parameters and WAV preprocessing](https://console.groq.com/docs/speech-to-text), [error codes](https://console.groq.com/docs/errors), and [Retry-After](https://console.groq.com/docs/rate-limits). Checked 2026-10-27.

## Shortcut and network recovery

The hook thread processes keyboard, mouse and desktop events. It recreates its low-level hooks on dashboard open, desktop switch and Windows resume, clearing stale held-key state without key polling. Only Cancel aborts a transcription. Open/Copy/Paste can revoke automatic delivery while preserving recognition; recording shortcuts during processing leave the current job intact.

Groq speech attempts share the caller's bounded total wait. Each request has an attempt deadline covering connection, headers and body. Transport errors, recoverable statuses and unusable JSON responses share three total attempts. Authentication failures and long rate limits are surfaced without repeated requests. Saved audio remains available for a deliberate retry.

Native notification geometry is computed from the current animated client bounds. Text is explicitly clipped above recovery buttons and the pill footer. Dashboard playback and notifications use one vertical feedback stack, with wrapping and scrollable long errors.

## Quick controls and prompt previews

The voice bar offers paste/copy, prompt improvement, history, settings, a one-hour widget snooze, restore, cancellation and quit. The tray shares the commands that require no editable-field target: copy, navigation, visibility, cancellation and quit. The widget uses a native popup menu created only on demand. Menu actions retain only the original window/process identity; text is read after an explicit prompt command, and paste still requires a fresh guarded target. Menus do not hold clipboard ownership.

Snooze is process-local and uses one deadline timer. It hides the resting widget without changing its dock, and active recording/processing feedback remains visible. Restore is available from the tray.

Prompt shortcuts bind the foreground window before queuing bounded whole-field capture. Frontend live events take precedence over older opening snapshots so completed previews cannot be replaced by stale loading state. New proposals scroll into view for editing and explicit Apply. Preference schema 7 migrates only the former default shortcut where the new chord is free; custom assignments and other settings remain intact.


## All-time insights and prompt editing

`insights.rs` streams every retained history row in chronological order, grouping activity into local calendar days, applications, hours and weekdays. Word counts come from saved text. Fixed English vocabulary counts form a writing-profile checkpoint at exact 10,000-word boundaries, including boundaries inside a transcript. Each checkpoint describes that block, rather than inventing a personality or biometric voice identity. Only recognized vocabulary, category counts and usage metadata enter the derived cache; there are no excerpts or cloud requests.

`history_totals` and SQLite insert/update/delete triggers maintain count, words and revision without rescanning transcripts on dashboard refresh. Existing databases are initialized once. The dashboard snapshot contains only 50 recent records; `history_page` searches/pages the entire database with stable ordering and a revision for consistent full exports. Text has no age cutoff. Audio retains its separate bounded policy.

Statistics run on a blocking worker with a separate read-only WAL connection and a consistent read transaction, leaving the recording actor's storage connection available. A version/revision/time-zone cache avoids repeated analysis. A new 10,000-word checkpoint schedules local rebuilding after delivery; startup can backfill existing history. Edits/deletions invalidate derived caches; private sessions never enter their source table. Statistics/History load data on demand and do no frame-driven or idle polling.

`prompt.rs` provides instructions and validation dedicated to prompt editing, separate from dictation cleanup. Both Groq and local Ollama use the prompt editor's system instruction. Clarify organizes existing requirements; Expand keeps proposed additions in explicitly optional sections. User editing preferences are bounded, encoded as data, and constrained by the chosen action. Number/link/backticked-identifier removal is rejected; this deterministic check does not establish general semantic correctness.

The sidebar prompt editor accepts transient input via `improve_prompt_text`; it never captures another field or creates a replacement target. Its `prompt_draft` preview can be copied, but cannot dispatch a native replacement. The shortcut still captures exactly the authorized whole field before opening Hush. A capture failure opens the editor with the reason instead of reading a surrounding document. `regenerate_prompt` always edits the retained original, including a new optional preference. Prompt contents are excluded from automatic history, insights and diagnostics. Both paths use the same cancellable 20-second inference deadline.

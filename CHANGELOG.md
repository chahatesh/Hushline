# Changelog

## 0.6.1 — 2026-10-28

- Refresh shortcut hooks on dashboard open, desktop switches and Windows resume; clear stale held-key state.
- Keep in-flight transcription and delivery alive when a recording shortcut is pressed again. Escape still cancels; opening Hush preserves the transcript.
- Retry malformed successful provider responses and give stalled speech requests a bounded attempt timeout within the existing total wait.
- Keep native notification text and recovery buttons inside their current animated bounds. Dashboard notifications wrap long errors and stack below playback controls.
- Simplify public documentation and remove maintainer setup handoffs and archived work logs. User, contributor, privacy, license and testing documentation remain.


## Signing integration — unreleased

- Prepare free SignPath Foundation signing with constrained app/uninstaller and outer-installer requests, trusted Authenticode/timestamp/content verification, and signed portable packaging. Enrollment and production signing remain pending.
- Keep the functional release gate; draft releases consume only verified signed files. Standalone signing builds produce artifacts without publishing a Release. Add signing and privacy policies.
- Test real NSIS export/import and throwaway in-memory Authenticode fixtures, tamper rejection and refusal of untrusted/test signers; no Windows root trust is changed.


## 0.6.0 — unreleased

- Protect every completed dictation recording before key lookup, network requests or text processing. Retain successful audio and expose Listen, Download WAV and Retry alongside its transcript; failed/unlinked recordings remain available in History and Home.
- Keep audio after successful or failed retries. Update the existing transcript instead of adding duplicate history/statistics. Delete linked audio with its transcript; Clear all removes persistent and memory recordings, including failures that produced no text.
- Encrypt saved audio with user-bound Windows DPAPI. Bound storage to 30 days, 1,000 recordings and 512 MB. Add a recording-storage switch; private mode/disabled storage use only the latest three memory clips, bounded to 64 MB. Downloads are explicitly requested, unencrypted WAV files. Cancelled new dictations are discarded.
- Raise the former five-second default transcription timeout to 30 seconds while preserving other configured waits. Longer recordings receive additional bounded time; manual retry gets at least 60 seconds, capped at 120. Fast replies never wait for the deadline.
- Retry interrupted/invalid response bodies as well as recoverable HTTP/network failures, within one maximum of three requests. Respect rate limits and cancellation, bound response size, retain fixed actionable failure reasons, and preserve speech captured before an endpoint failure. Quiet captures remain available for listening/manual retry and are never automatically uploaded.
- Keep existing app/storage identities, provider/key, history, shortcuts, writing styles, private mode and independent AI settings. Old transcript and saved-recording metadata remain readable.

## 0.5.3 — unreleased

- Repeated recording shortcuts while audio is finishing or text is processing no longer cancel and discard the in-flight transcript. Finish the existing delivery first; Escape remains explicit cancellation.
- Ordinary dictation now inspects the focused editable field at delivery inside the original window/process. Failed early inspection, missing provider runtime IDs, and app-managed field replacement no longer automatically discard paste authority. Actual typing/clicking, window changes, cancellation, protected fields and terminals remain guarded.
- Keep window/process/input authority through recording, transcription and delivery. Skip unnecessary early UIA for normal dictation; optional context uses its own bounded slot and cannot occupy delivery slots. Read-only inspection retries happen before clipboard ownership, never after a paste/edit dispatch.
- Retain strict original-selection checks for Command Mode, approved previews and saved-audio recovery. Preserve snippet confirmation with an on-demand strict field snapshot. Allow scrolling without cancelling delivery, and give held modifiers up to three seconds to release within the existing delivery deadline.
- Store a fixed failure code alongside failed history rows when history is enabled. Old rows remain compatible; no field contents, provider error payloads or keys are added to diagnostics.
- Add code-only regressions for failed/slow early inspection, the recording-to-delivery handoff, window/process/input guards, command selection waiting and history compatibility.

## 0.5.2 — unreleased

- Fix a reproduced browser paste failure when the focused text box grows or moves during dictation. Ordinary dictation now validates the same field, a collapsed selection and the real input generation rather than requiring identical screen coordinates.
- Inspect selection endpoints without reading field contents. Newly selected text, changed fields, physical edits, protected inputs and cancellation still prevent insertion; preview replacements and native selection checks remain strict.
- Extend real Windows/Chrome tests to cover stable fields, layout changes after target capture and programmatic focus changes, verifying complete multiline delivery exactly once and no insertion into another field.

## 0.5.1 — unreleased

- Prioritize automatic delivery of ordinary dictation, including available speech after microphone interruptions. Keep interrupted Command Mode edits blocked, and retain changed-target/protected-input safeguards.
- Stop treating provider-generated accessibility focus notices as user edits. Recheck the original field, actual keyboard/mouse activity and foreground changes before delivery.
- Fall back silently to local text when automatic AI cleanup fails or changes protected facts; keep the fallback outcome in content-free diagnostics.
- Normalize clipboard line endings to Windows CR-LF without doubling existing separators.
- Allow more time for field inspection and clipboard availability without automatic repeat dispatch. Keep error/recovery/warning notices above the pill for 12 seconds, and dashboard notifications for 6/15 seconds. Recording details still compact after 2.5 seconds.

## 0.5.0 — unreleased

- Insert into supported Unicode Windows edit controls in one undoable native operation, without typing or changing the clipboard. Recheck the field, selection, cancellation and input generation; uncertain delivery is never automatically repeated.
- Preserve native bitmap and enhanced-metafile clipboard images alongside supported text/rich-text formats. Bound snapshots to 32 MiB per format and 96 MiB total; refuse unsupported formats before mutation and protect newer user copies.
- Add **Paste here** and **Copy text** to recovery cards above the pill. Prevent stale copy/paste results and hide timers from replacing newer feedback.
- Honor explicit numbered-list commands in Casual, Standard and Formal, preserving multi-sentence items, surrounding prose and sequential numeric items beyond ten. Exact-word behavior stays unchanged.
- Cancel abandoned preparation quietly, fix light-theme native text contrast, and show the selected speech model on Home. Preserve existing shortcuts, profiles, identifiers and credentials.
- Add real cross-process Windows edit/Undo/protection tests, image ownership and clipboard checks, recovery hit-test/nonactivation checks, and list regressions. Live microphone-to-Groq-to-Notepad/browser acceptance is still required before a public Release.

## 0.4.2 — unreleased

- Keep the original text field authorized when the nonactivating pill is clicked to stop. Ordinary clicks, scrolling, focus changes, protected fields, and cancellation still prevent unsafe delivery.
- Count only short taps toward optional double-tap activation. A second activation held for dictation now ends on release; finishing hands-free by press-and-release requires no extra click. Normal holds no longer wait through the double-tap window.
- Add bounded, content-free reason codes for target inspection and delivery failures to the existing in-memory diagnostics export.
- Strengthen actual Windows insertion checks with asynchronous delivery waits, owned test-field guards, and an interactive browser test. Live microphone-to-Groq-to-target acceptance remains a release requirement.

## 0.4.1 — unreleased

- Center the timer in both expanded and compact side docks; keep the bottom timer left-aligned.
- Ignore accessibility focus events from background windows when deciding whether the original field changed. Foreground changes and actual input still invalidate automatic insertion.
- Validate clipboard publication after closing it, and check both clipboard owner and sequence before paste/restoration so a newer user copy remains protected.

## 0.4.0 — unreleased

- Move the voice-bar timer to the left and × to the right, including compact mode; side docks put time above the centered waveform. Keep preparation and processing dark, with distinct stage labels; only microphone samples drive the bright recording waveform.
- Add a configurable 300 ms startup pause (0–1,500 ms) and an optional quiet, original recording-ready cue on first frames. Abandoned holds cannot start recording later. Preserve existing double-tap preferences and its separate release window.
- Retain pending original-target inspection across recording release, separate transcription from insertion invalidation, and bound microphone startup/delivery. Opening Hush or copying during processing must retain recoverable text rather than leave a stuck processing state. Keep clipboard restoration and late-result safeguards.
- Add Casual, Standard, and Formal writing styles independently of Always/Sometimes/Never AI polish, with exact-word preservation under Advanced transcription. Migrate legacy profiles without resetting provider, shortcuts, double-tap, history, key, or timeout choices.
- Strengthen explicit self-correction handling, mixed prose/list parsing, multi-sentence items, and mode-specific AI instructions. Validate against corrected meaning, protect unrelated facts/identifiers, and retain local text when AI output is rejected or times out. Ambiguous wording remains conservative; universal semantic accuracy is not claimed.
- Add shortcut-only Improve prompt (Ctrl+Win+P): read one complete focused editable field up to 32 KB, Clarify or Expand with a 20-second AI deadline, and review/edit before Apply/Copy/Cancel. Optional additions and assumptions are labeled. Revalidate the unchanged original field before a single approved replacement; preserve unsupported/changed fields. Prompt contents stay transient and out of history/diagnostics. Proposed output is limited to 100 KB.
- Add bounded, memory-only stage timing/outcome diagnostics with explicit export, plus migration, state/deadline, writing, prompt-edit, and interface regressions. See docs/QA.md for actual executed results.
- Recording cues, revised native behavior, whole-field prompt support, and uninterrupted live Groq-to-application acceptance remain partially verified until the required Windows checks pass. The public GitHub Release gate remains closed.

## 0.3.3 — 2026-10-20

- Simplify the voice bar to a centered waveform with the time beside ×; remove the listening label. Close remains usable after the 2.5-second contraction.
- Add a five-second, configurable speech deadline with the requested error above the pill. Cancel expired work so late results cannot paste.
- Keep failed audio for Retry/Delete in Home, encrypted using Windows DPAPI. Retain at most three clips for seven days; private-mode audio stays in memory. Successful retries discard saved audio and return text for review/copy.
- Bound optional cleanup waits and fall back to local formatting. Add deadline, retention, private-storage, browser and native regressions.
- Full live dictation acceptance and the public release gate remain pending; see docs/QA.md.

## 0.3.2 — unreleased

- Correct the overly strict 0.3.1 audio policy: a brief discontinuity no longer disables paste, snippets or AI polish for the entire recording. Require review for measured loss of at least 250 ms in one gap or 500 ms in total. Missing audio is not reconstructed.
- Set the native pill's arrow cursor explicitly, including client-area cursor messages, to avoid inherited busy cursors.
- Collapse listening/processing details after 2.5 seconds into a centered, opaque waveform while recording continues. New recordings and new notices briefly expand again; meter updates do not reset the timer. Guard queued timer messages from older sessions.
- Center the expanded waveform too, place the timer beside it, and dismiss completed/error states after at most 2.5 seconds. Recovery text remains in Home until the next recording.
- Add packet-timing diagnostics without saved/uploaded audio, brief/substantial-loss policy regressions and actual native cursor/collapse checks.

## 0.3.1 — unreleased

- Unfold the native pill upward from a fixed lower edge as one rounded surface. Status and recovery details stay inside it; bottom and side docking retain their anchors.
- Use a more vibrant violet/turquoise palette in the native bar, dashboard, previews and original app icon; retain light/dark themes and reduced motion.
- Deliver microphone activity to the native bar at up to 50 Hz, coalesce pending paint messages, and limit dashboard meter messages to 10 Hz. Reuse drawing surfaces and cached titles rather than allocating them every frame. Idle has no animation timer.
- Keep whole, distinct vocabulary hints within the request budget instead of cutting through dictionary names or letting one oversized rule crowd out later entries.
- Recover the remaining audio after a microphone packet gap instead of ending the recording. Display a warning immediately, preserve the transcript for review, and block automatic paste, snippet expansion and AI polishing. Interrupted edit commands are rejected. Missing speech cannot be reconstructed.
- Add anchor/easing, drawing-reuse, dictionary-budget and interrupted-capture guards, plus real native 50 Hz workload and forced audio-overrun checks. Full speech-to-target and mixed-monitor acceptance remain open.

## 0.3.0 — unreleased

- Rename the visible product to Hush while retaining the application identifier, database, credential slot and executable for existing profiles.
- Isolate installer registry identity and Hush Dictation shortcuts from unrelated apps with the same name; retain the upstream Tauri template's MIT attribution.
- Show wrapped errors and recovery warnings above the native pill at bottom/left/right docks, without activating the window. Keep notices with recovered transcripts visible longer.
- Add Always / Sometimes / Never AI polish choices. Never overrides automatic app styles; exact words and snippets remain exact. Offline mode stays local.
- Improve list introductions within prose and preserve/canonicalize explicit Markdown bullet lines. Keep identifiers and code intact.
- Remove terminal periods only from clear English fragments; keep sentence punctuation and avoid appending periods unconditionally. Give the AI the same punctuation/paragraph/list rules.
- Add cold-start/normal-stop microphone checks, native error-surface checks, policy migration tests and real synthetic Groq text-model checks. The reported intermittent first-use error has not been reproduced conclusively.


## 0.2.1 — unreleased

- Remove the microphone drawing from the voice bar, reduce its footprint, and replace clipped window-region corners with smooth per-pixel alpha.
- Prepare a stopped native WASAPI client in advance for faster recording activation. Add a setting to disable preparation; no audio is read while idle.
- Drain final frames, reset between sessions, detect packet loss/device stalls, and keep encoding independent of the next recording.
- Raise the old two-minute default to ten minutes and make silence stopping opt-in so thinking pauses do not end hands-free dictation.
- Improve paragraphs within existing line breaks, everyday list introductions, repeated words/restarts, and natural interface wording.
- Acknowledge paste before its background clipboard restoration delay; retain format/sequence/focus safeguards and cleanup warnings.
- Guard optional AI edits against missing content/identifiers/numbers, reduce supported reasoning overhead, and prevent local context truncation.
- Benchmark a small local text model without installing a service or changing the user's Ollama configuration. Keep AI processing optional.
- Fix portable-package hashing in the hosted Windows build using the standard .NET SHA-256 implementation.

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

# Quality and release verification

Hushline 0.1.0 is not declared production-complete. The release gate remains closed. Tests below distinguish pure/native components, UI review, real OS integration, and live provider acceptance.

## Executed automated checks

- Rust backend: 29 tests passed, 0 failed, with 4 interactive/device/credential tests ignored in the ordinary suite. This includes the six loopback HTTP cases: documented multipart fields, recoverable retry, long 429, 401 body redaction, pre-cancelled requests, and truncated AI output rejection. Additional tests cover safe normalization/identifiers, non-chained dictionary replacements, exact snippet matching, resampling quality, SQLite persistence/deletion, invalid-restore rollback, settings/key exclusion, chord validation, and Escape behavior.
- TypeScript unit tests: 4 passed.
- Browser UI: 6 passed against the compiled frontend in Playwright Chromium. Tests cover truthful empty state/native boundary, dictionary CRUD/reload, multiline snippets, writing style creation, scratchpad persistence, zero-usage statistics, settings/private mode/theme, backup exclusion, keyboard focus, compact layout, and author/license presentation.
- TypeScript compilation and production frontend build passed. Initial compressed output was approximately 27 KB JavaScript and 6 KB CSS; these are build sizes, not runtime-memory claims.
- Rust Clippy with warnings denied passed in the recorded run. Formatting is applied with rustfmt. The final source was checked again before packaging.

Loopback fixtures use a local server and dummy keys. They verify request/response/error handling, not Groq accuracy, account access, billing, or real provider latency. The browser preview stores review-only collection data separately and does not simulate microphone/input/native key behavior.

## Real Windows integration attempted

The ignored `native_input_clipboard_focus_and_password` test opens owned native edit controls, snapshots supported clipboard formats, and would test Unicode delivery, paste restoration, cancellation, changed foreground window, and password refusal. Windows refused its foreground activation in this execution environment. The test stopped at **“Interactive desktop focus could not be acquired” before any text injection**. It did not bypass Windows restrictions. This is not counted as a passing insertion/clipboard/password test. The owned clipboard sentinel was guarded and restored on test exit.

Actual microphone activation/cancellation and Credential Manager dummy-key round-trip have separate ignored smoke tests; their final results are recorded in the verification appendix below. Neither test uploads audio or authenticates with Groq. They do not replace live dictation acceptance.

Early validation issues were corrected: a syntax delimiter and two Windows binding argument mismatches; test environment separation for Vitest; the idle Escape interception and cancellation/insertion race; loopback network permission for fixtures; and browser screenshot stability by using Playwright's dedicated browser. An earlier linker file-lock error was caused by a still-running stalled test process and resolved by stopping only that workspace-owned process. No failing check was reclassified as a pass.

## Required manual acceptance protocol

Run on Windows 10 and 11 x64 with ordinary user permissions. Record OS build, CPU/RAM, microphone, network conditions, Groq model, target app/version, test date, and actual result. Keep private speech out of logs and test artifacts.

| Scenario | Procedure | Current result |
| --- | --- | --- |
| Install/start | Install per-user package, launch, verify tray, dashboard, About version/license. | Unverified |
| Key setup | Save a valid key, restart, verify models, replace/remove it, confirm key absent from DB/assets/logs. | Live key unverified |
| Microphone | Select devices, test levels, reconnect/unplug mid-recording, deny privacy permission. | Full matrix unverified |
| Push-to-talk | Focus Notepad, hold Ctrl+Win, speak a known sentence, release, compare exact inserted text. | Unverified |
| Hands-free | Toggle on/off, check timer/levels, allow sustained silence and max duration. | Unverified |
| Double-tap | Enable, activate twice quickly, then finish with hands-free shortcut. Repeat near timing threshold. | Unverified |
| Rapid cancel | Repeat start/release/Escape/reactivate at least 20 times during capture/network/preview. No stale insertion. | Unverified |
| Silent recording | Record silence in quiet/noisy conditions. Confirm no unwanted cloud request or hallucinated insertion. | Unverified |
| Network failure | Disconnect before/during upload, restore, verify bounded retry, recovery message, cancellation. | Real provider unverified |
| Groq errors | Use invalid key/model and a legitimately rate-limited account; verify actionable messages without key leakage. | Fixtures pass; live unverified |
| Focus/caret change | Speak in app A, switch or move cursor before completion. No insertion into A or B; recover transcript. | Unverified |
| Clipboard | Test Unicode leaves clipboard untouched; paste restores text/HTML/image/registered formats when supported; copy newer content during pending restoration. Unsupported snapshots must be refused. | Unverified |
| Password/protected | Attempt capture in native and browser password/read-only fields and elevated apps. No insertion/readback/bypass. | Unverified |
| Applications | Notepad, Word, Chrome/Edge/Firefox fields, Google Docs/Gmail, Discord/Slack, VS Code/Cursor/Windsurf. Verify spaces, punctuation, Unicode, selection, multi-line input. | Unverified |
| Dictionary | Dictate names/acronyms; check vocabulary hints and exact replacement boundaries. | Local tests pass; live unverified |
| Snippets | Dictate exact trigger vs a sentence containing trigger; test multiline, date/time, confirmation/discard. | Local/UI tests pass; live unverified |
| Command Mode | Opt in, select own sample text, speak shortening/formatting instruction, review, approve. Change selection before approval and confirm refusal. | Unverified |
| Meaning/formatting | Test Literal/Clean/Polished with dates, identifiers, URLs, acronyms, repeated words, ambiguous self-corrections, multilingual speech. | Local regression subset passes; live unverified |
| History/private | Dictate with history on/off; switch off while processing; restart; search/export/delete/clear. No private transcript row. | Storage/UI tests pass; live unverified |
| Overlay/multi-monitor | Test bottom placement, negative coordinates, mixed DPI, light/dark/reduced motion; ensure overlay never takes focus. | Native unverified |
| Lifecycle | Close/reopen dashboard, single instance, startup opt-in/off, quit during recording/network. | Unverified |
| Update/uninstall | Upgrade over existing profile, verify settings, uninstall shortcuts/startup, deliberate data retention/deletion. | Unverified |
| Independent build | Clean Windows machine/VM, prerequisites, npm ci, all checks, package, install. | Unverified |

Only after actual core checks pass should `release-verification.json` be completed with evidence. `node scripts/verify-release.mjs` intentionally fails on the current unverified report. CI compilation and mocked HTTP/UI tests alone cannot open this release gate.

## Verification appendix

The actual release executable passed a native desktop smoke run through its real Tauri IPC bridge: snapshot, persistent dictionary add/reload/delete, dummy credential entry/removal in the UI, microphone-test cancellation, and backup exclusion. No browser-preview storage was used for these checks. The dummy key and dictionary entry were removed. Observed launch-to-heading time was 18,990 ms including test-only CDP initialization under build load; it is not a pure cold-start benchmark. A subsequent final-build run is recorded with the packaged artifact results when available.

The actual microphone test passed with 2,869 ms activation to stream-ready in its debug run, and the separate Credential Manager round-trip passed in 0.11 seconds of test execution. These do not constitute live Groq acceptance. The frozen-source ordinary suite reached 29 passed with 4 ignored tests, including identifier protection, multilingual safeguards, multiline no-submit behavior, and cancellation before microphone activation. See PERFORMANCE.md and the final validation record for the exact final totals.

The first optimized executable and NSIS installer were generated successfully (7,456,768-byte executable; 2,662,139-byte installer). They are being regenerated after final fixes. Installation, clean update/uninstall, real Groq speech, actual system-wide text delivery, and broad compatibility are still unverified. Missing results remain unverified; no success should be inferred from code presence.

The native overlay appearance/nonactivation test passed, including forced painting of its empty initial state. A real startup exception was traced to an empty UTF-16 text buffer being passed to Windows DrawText; the guard and terminated buffer fixed it, and debug background startup then stayed alive. The release package is validated again below before handoff.

## 0.1.0 packaged-build baseline

- Frozen-source Rust suite: **29 passed, 0 failed, 4 ignored** in the ordinary suite; Rust formatting and Clippy with warnings denied passed.
- TypeScript unit tests: **4 passed**; interface tests: **6 passed**; TypeScript compilation and frontend lint passed.
- Separately executed real-device tests passed: microphone frames/cancellation and dummy Credential Manager save/read/delete. The real native overlay passed show/hide/no-focus-change and empty-initial-paint regression tests.
- The final release executable passed its real Tauri IPC/UI tests: dictionary persistence and deletion, secure-key entry/removal, key exclusion from snapshots, microphone-test cancellation, backup exclusion, closing the dashboard while retaining the tray engine, and reopening through a second launch. No live Groq inference was used or represented as tested.
- Final observed dashboard launch-to-heading time: 3149 ms, with test-only CDP overhead. Final background working set: 16.61 MiB; see PERFORMANCE.md for methodology and limits.
- The real NSIS package passed guarded per-user **silent install → installed background launch → uninstall** in a new owned directory. Dependency licenses were present and the local profile was retained. The interactive wizard and upgrade path are not certified by that test.
- **Still unverified:** live Groq speech, actual transcription-to-text delivery in Notepad/other apps, clipboard restoration after real insertion, protected-input compatibility, live Command Mode, multi-monitor/DPI scenarios, and a second-machine clean build. The interactive insertion test stopped when Windows refused foreground activation. The release gate remains closed.

The sanitized JSON reports in verification/ are evidence for these specific tests. They are not live provider or full end-to-end acceptance results.

## 0.2.0 update — 2026-10-19

The shortcut overlap was reproduced as release-edge fall-through from HandsFree/Command/Paste to Push. The router now emits activation only on fresh key-down edges and preserves consumed key-up pairs. Regression coverage includes 100 rapid presses, modifier release order, plain Space, Escape while idle/active and more-specific chord release. The capture silence condition also incorrectly applied to hold-to-talk; it is now gated by the hands-free state. A stopping capture can be replaced by a new quick press.

Executed checks: 39 Rust unit tests, 5 TypeScript tests and 8 browser UI tests passed; formatting and Clippy with warnings denied passed. Browser checks cover the offline selection, automatic layout, default paste, dock persistence and preview, and recorded/preset shortcuts, in addition to the original CRUD/private-mode/backup tests. An initial browser cold-start timeout was removed by eliminating a redundant setup navigation in fresh test contexts. A select label was made explicit. Visual inspection caught and corrected a CSS layer-order regression before delivery. Screenshots were reviewed after the fix.

The real offline test downloaded and SHA-256-verified the official CPU engine and Tiny model, piped the upstream JFK WAV into the engine, parsed real output and verified a phrase from the recording. It passed. One transcription measured 2,751 ms for the approximately 11-second speech fixture; this does not prove live microphone-to-application or cloud acceptance. Raw audio was not saved by the pipeline. Actual Ollama rewriting remains unverified.

The public repository was created with the owner's authorization at https://github.com/chahatesh/Hushline. Installer, native runtime and idle measurements for the packaged revision are recorded below when executed. No public release tag is created while core acceptance is incomplete.

Native bar validation passed: no foreground activation, 216×40 logical-pixel bottom active size, 40×156 side active size, 24×64 side resting size, alpha 255 while active, and the hidden-idle option. Real microphone frames and cancellation passed again, with one 2,221 ms stream-ready observation while release compilation was active. This does not meet the startup ambition and is not presented as a latency guarantee. The native insertion test was reattempted and Windows again refused foreground acquisition before text dispatch; paste/restoration remains unverified in a real target application. The clipboard sentinel guard ran on exit. No focus/security restriction was bypassed.

### Packaged 0.2.0 verification

The final optimized executable and NSIS package built successfully. Installer: 2,793,040 bytes, SHA-256 `05681c9de727683ebd77a8ca57f41002d6375192df74d1a04c9e3211058d18c5`. A portable ZIP with original and dependency licenses was also generated.

- Real Tauri IPC/dashboard smoke passed: snapshot/secret exclusion, dictionary add/reload/delete, microphone frames/cancellation without a history entry, backup exclusion, close-to-tray and reopen through a second launch. An existing API key was detected and preserved; it was not read, replaced or used for a cloud request. The native screenshot masks sensitive history/statistics regions.
- The actual NSIS package passed per-user silent installation, installed background launch, dependency-license presence, and uninstallation in a new workspace-owned directory. The user profile was retained. Interactive wizard, upgrade and broad application compatibility remain unverified.
- Fresh-background working set averaged 15.94 MiB, private memory 3.46 MiB; CPU rounded to 0.000% across all logical processors. Dashboard heading observed at 3896 ms, including test setup overhead. Methodology and limits are in PERFORMANCE.md.
- The latest instrumented real microphone run reached stream-ready in 2,006 ms (device 12 ms, format 13 ms, stream built 2,005 ms). This identifies a Windows stream-creation delay; the startup ambition remains unmet.
- Release verification now compares evidence with the package version rather than a stale literal. It correctly rejects the incomplete acceptance report. No release tag or GitHub Release was created.

See [native desktop](verification/desktop-smoke-0.2.0.json), [installer](verification/installer-test-0.2.0.json), and [update record](verification/update-0.2.0.json). Live provider-to-application delivery, real paste/restoration, actual Ollama editing, Base accuracy, mixed-DPI monitors and independent clean-machine acceptance remain open.

## 0.2.1 update — 2026-10-19

The recording delay was isolated to repeated stream initialization. A native stopped-client experiment prepared in 1,179 ms; three direct Start trials produced first frames in 25/8/7 ms. The integrated worker's final three prepared starts reported Ready in **21/5/4 ms** and actual frames in **21/68/61 ms**, with successful cancellation each time. These are specific hardware/debug observations after preparation, not universal instant-start claims. Preparation has an off switch; the idle worker reads no audio and blocks on commands.

The final ordinary Rust suite passed **49 tests** with eight explicit device/provider tests ignored; formatting and Clippy with warnings denied passed. TypeScript tests passed **5**, browser interface tests **9**. The restricted browser run could not reach its preview server and was stopped; the normal desktop run passed. Reproduced formatter failures covered natural list introductions, long paragraphs containing line breaks, and repeated words/restarts. A synthetic 125-second WAV preserved its exact final sample. A full long spoken session still needs manual acceptance.

The native overlay passed per-pixel surface acceptance, active opacity, dimensions, no foreground activation and idle hiding. Fractional alpha/premultiplication tests passed and actual native rendered pixels were inspected. The mic symbol is absent. The real insertion harness still failed safely at foreground acquisition before dispatch; clipboard/new asynchronous completion compatibility remains unverified.

A **real Groq** request using the saved key transcribed the public upstream JFK fixture in **2,542 ms**, returning 22 words and the expected phrase. The key was neither printed nor replaced, and no private microphone audio or history was uploaded. This proves that fixture/provider path, not microphone-to-Notepad acceptance.

The qwen3:0.6b feasibility benchmark used synthetic text, a separate cloud-disabled service and workspace-only weights; it measured significant active RAM and 3–15 s edits. Its service was stopped. The longer fixture did not gain paragraphs. See LOCAL_MODEL.md and verification/local-model-0.2.1.json; this trial does not establish broad model quality.

The previous hosted run passed its functional checks, real offline fixture and installer build, then failed because its child PowerShell could not resolve Get-FileHash during portable packaging. The portable script now uses standard .NET SHA-256 directly. No failure was relabeled as success; the next hosted result and packaged-runtime evidence are recorded below when available. No release tag is created while core acceptance is open.

### Packaged 0.2.1 result

The optimized executable (7,715,328 bytes) and NSIS installer (2,802,071 bytes) built successfully. The portable ZIP and its .NET SHA-256 calculation also completed.

The actual release dashboard passed secret exclusion, dictionary add/reload/delete, key preservation, microphone cancellation, normal Stop/final-frame drain with safe silence rejection, backup exclusion, close-to-tray and reopen. Neither microphone test uploaded audio or added history. Observed heading time was 6381 ms including test-only CDP overhead. The native screenshot masks private history/statistics.

Guarded per-user silent install, background launch, dependency-license presence, uninstall and retained profile passed in a new owned directory. The interactive wizard and update matrix remain unverified. Fresh-background memory averaged 18.95 MiB working set and 4.16 MiB private; CPU rounded to 0.000% across all processors. These are one-run observations, not hardware-wide guarantees.

Reports: [native desktop](verification/desktop-smoke-0.2.1.json), [installer](verification/installer-test-0.2.1.json), [performance](verification/performance-0.2.1.json), [update](verification/update-0.2.1.json). The source was published in commits 8544c35 and aef7f55. Live microphone-to-target, clipboard restoration after real insertion, full local-edit quality, long live speech, mixed-DPI monitors and independent manual installation remain open. No GitHub Release was published.
## Hush 0.3.0 verification — 2026-10-20

Rust: 54 passed, 10 explicitly ignored device/live tests in the ordinary suite; no failures. TypeScript: 5 passed. Browser dashboard: 10 passed, including Hush branding, Always/Sometimes/Never selection, persistence and exact-word pause behavior. Formatting and Clippy with warnings denied passed.

The actual release executable passed native snapshot/secret exclusion, all three policy settings through native IPC (then restored the existing preferences), dictionary persistence, saved-key preservation, microphone Test cancellation and normal Stop with safe silence rejection, backup exclusion, dashboard close-to-tray and reopen. The test uploaded no microphone audio and added no history. Dashboard heading observation was 3440 ms including test-only CDP setup.

The final native overlay test painted wrapped error/recovery cards at bottom/left/right docks, retained focus and full active opacity, and passed actual layered-window painting. The synthetic native pixel capture was inspected. Mixed-DPI monitors still require manual checks.

A final local microphone run passed one cold first use followed by three reused starts, all with normal Stop: Ready 666/4/6/3 ms. A separately prepared worker passed starts/cancellations with Ready 8/6/8 ms and first frames 73/59/61 ms. No audio was saved or uploaded. **The user's intermittent first-use red state was not conclusively reproduced.** The card now displays the actual reason and recovery warning; no claim is made that an unidentified hardware, network or insertion failure has been fixed.

Two real Groq `openai/gpt-oss-20b` requests used synthetic text only. Sometimes and Always edits passed structure/content/identifier/number checks, taking 567/671 ms. Never skipped the edit request. These are editing-step observations, not full dictation timings or a general accuracy guarantee.

The first installer test correctly refused a collision with an existing unrelated Hush 0.1 registration. The installer now vendors the exact MIT-licensed Tauri CLI 2.12.3 template, changing only the registry identity and shortcut names. The final package passed guarded per-user silent install, background launch, dependency-license presence, uninstall and profile retention in a fresh owned directory. Existing Hush registration and shortcut hashes remained unchanged. The interactive wizard and upgrade matrix remain unverified.

The new installer and portable ZIP were built. The final background benchmark measured 18.89 MiB mean/peak working set, 4.07 MiB private memory and CPU rounded to 0.000% across 12 logical processors. See [native desktop](verification/desktop-smoke-0.3.0.json), [installer coexistence](verification/installer-test-0.3.0.json), [performance](verification/performance-0.3.0.json) and [formatting checks](verification/formatting-0.3.0.json).

The source is public. Live microphone → Groq → target insertion, real clipboard restoration, selected-text edits, long live speech, multi-monitor acceptance and an independent manual build remain open. No release tag or GitHub Release is published.

Final hosted Windows CI passed for implementation commit f4a464d, including lint, tests, real offline speech fixture, installer packaging and portable artifact upload: https://github.com/chahatesh/Hushline/actions/runs/39415266772. Documentation-only commits do not alter that tested implementation.

## Hush 0.3.1 verification — 2026-10-20

Rust: **57 passed, 12 explicitly ignored** native/device/provider tests in the ordinary suite. Formatting and Clippy with warnings denied passed. TypeScript: **5 passed**; browser: **11 passed**, including upward preview expansion and the stronger light/dark palette. The preview test first used viewport coordinates, which changed when Playwright scrolled; it now checks layout coordinates relative to the preview parent. That test correction is not a native geometry fix.

Real native overlay tests verified a fixed lower edge while expanding at bottom/left/right docks, initial animation geometry, full active opacity, unchanged focus and reuse of the allocated drawing surface. Actual rendered native pixels were inspected. Geometry tests cover 1×/1.5×/2× scales and negative monitor coordinates; this does not replace physical mixed-DPI monitor acceptance. A five-second synthetic 50 Hz workload delivered 244 readings and used 0.1719 CPU seconds (0.286% across 12 logical processors) in the debug test process without additional steady-size surface allocation. It excluded microphone capture, network and the dashboard.

This time a real WASAPI packet gap reproduced the earlier audio-loss error path. A controlled 300 ms reader pause reliably forced a 200 ms endpoint overrun. Larger endpoint buffers and audio-thread priority did not reliably remove the device/scheduling gaps; those experiments were reverted. The final recorder retains subsequent packets and marks the capture for review instead of aborting it. The forced-overrun test passed with one marked gap and 13,014 retained/padded frames. An engine regression blocks automatic insertion after any gap. Interrupted captures also skip snippets/automatic AI editing, and interrupted Command Mode stops before transcription/editing. The original missing speech cannot be reconstructed. A live interrupted speech-to-review session still needs manual acceptance.

Final real microphone checks passed one cold-first-use plus three reused normal Stops: Ready **249/6/4/16 ms**. A separately prepared worker passed three starts/cancellations: Ready **8/3/7 ms**, first frames **38/120/42 ms**. These are debug observations under variable machine/compiler load; the 120 ms sample is retained and no universal sub-100 ms claim is made. Tests saved/uploaded no microphone audio.

One real Groq Turbo request transcribed only the public upstream JFK WAV in **1,342 ms**, with 22 returned words and the expected phrase. The existing key was neither printed nor replaced. The fixture SHA-256 is `59dfb9a4acb36fe2a2affc14bacbee2920ff435cb13cc314a08c13f66ba7860e`. This proves that fixture/provider path; it is not a general accuracy result or a microphone-to-target benchmark. Complete/distinct vocabulary-hint regressions pass, but no word-error-rate study has been performed.

Hosted Windows CI passed for implementation commit `e281013`: frontend/Rust checks, the real CPU offline speech fixture, NSIS installer, portable packaging and artifact upload. [Run and step evidence](https://github.com/chahatesh/Hushline/actions/runs/39515795860), [saved record](verification/ci-0.3.1.json).

The final optimized executable (7,739,392 bytes), NSIS installer (2,810,331 bytes) and portable ZIP (3,544,628 bytes) were built. Actual release IPC checks passed secret exclusion, all three polish-policy settings with restoration of the original preferences, dictionary persistence, existing-key preservation, microphone Test cancellation, normal Stop with silent-recording rejection, configuration backup exclusion and close-to-tray/reopen. No microphone audio was uploaded and no history was added. The dashboard heading appeared after 3,360 ms including test-only CDP setup; this is not a pure startup percentile.

The final installer passed per-user silent installation in a fresh owned directory, background launch, dependency-license presence, clean uninstall and existing-profile retention. The unrelated legacy Hush registration/shortcut hashes were unchanged. Interactive wizard and upgrade compatibility still need manual acceptance.

One fresh-background run, after compiler/test jobs ended, measured **18.99 MiB mean/peak working set**, **4.09 MiB private memory** and **0.000% CPU at the sampling resolution** across 12 logical processors. The native resting pill and stopped microphone preparation were enabled. There was one process, no dashboard and no local inference model. This is not a recording/inference or open-dashboard measurement.

Reports: [native desktop](verification/desktop-smoke-0.3.1.json), [installer](verification/installer-test-0.3.1.json), [performance](verification/performance-0.3.1.json), [binary hashes](verification/artifacts-0.3.1.json) and [component checks](verification/update-0.3.1.json). Live microphone-to-target insertion, real clipboard restoration, selected-text replacement, interrupted speech-to-review, mixed-monitor behavior and independent clean-machine acceptance remain open. The public release gate correctly rejects the incomplete acceptance report; no release tag or GitHub Release was created.

## Hush 0.3.2 correction — 2026-10-20

The report that every recording was held for review exposed an overly strict policy in 0.3.1. Timing-only real-device diagnostics reproduced a startup discontinuity of 5,887 frames at 48 kHz (about 123 ms). The previous policy treated any discontinuity as permission to block insertion, snippets and AI editing. The new gate uses measured missing duration: **250 ms in a single gap or 500 ms in total**. Brief/untimed discontinuity flags alone do not block normal dictation. This policy does not reconstruct missing speech or prove that short gaps lose no words.

The brief-hiccup policy regression failed under the old rule and now passes. Three five-second real microphone captures passed without review; the first contained an 81 ms gap and the next two had no measured gaps. Nothing was uploaded or saved as an audio file. A properly warmed stream paused for 700 ms with a 200 ms endpoint, then drained subsequent packets, reported **24,000 missing frames / 500 ms** and correctly required review while retaining 42,720 frames including padding. The initial overrun test checked too early and could confuse a startup discontinuity with the forced overrun; it now separates warmup and waits for the later flagged packet.

The native cursor regression first failed because the class cursor was null. Explicit arrow registration and WM_SETCURSOR handling now pass. The timed-collapse regression first found the old 168×48 layout after three seconds; it now reaches a **96×28 centered waveform** after 2.5 seconds, retains the lower edge and listening phase, and does not activate the target window. Side compact geometry is **28×88**. New sessions/notices expand again; meter updates and listening-to-processing transitions do not rearm expansion. A queued early timer is checked against the current deadline. Errors/completed recordings return to idle within 2.5 seconds while recovery information remains in Home.

Rust: **59 passed, 14 explicitly ignored** native/device/provider tests in the ordinary suite; TypeScript: **5 passed**; browser: **11 passed**. Formatting and Clippy with warnings denied pass. An initial restricted run could not connect to its loopback HTTP fixtures and hung; its owned test process was stopped and the complete unrestricted suite passed. One fixed-delay native geometry snapshot ran before animation progress was delivered; the assertion now waits for the actual bounds within a bounded interval. No failed run is counted as a pass.

Busy waveform traffic also exposed late timer/animation delivery. Collapse deadlines and animation progress now advance during native meter updates as well as timers, avoiding reliance on lower-priority WM_TIMER delivery. See [Windows timer semantics](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-timer). The native test synchronizes posted updates before reading geometry; its initial-size check waits for settled bounds within a deadline.

Packaged validation and hosted CI are recorded below after execution. Actual speech-to-target insertion/clipboard restoration and physical mixed-monitor acceptance remain unverified; the public release gate stays closed.

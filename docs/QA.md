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

## Final packaged-build result

- Frozen-source Rust suite: **29 passed, 0 failed, 4 ignored** in the ordinary suite; Rust formatting and Clippy with warnings denied passed.
- TypeScript unit tests: **4 passed**; interface tests: **6 passed**; TypeScript compilation and frontend lint passed.
- Separately executed real-device tests passed: microphone frames/cancellation and dummy Credential Manager save/read/delete. The real native overlay passed show/hide/no-focus-change and empty-initial-paint regression tests.
- The final release executable passed its real Tauri IPC/UI tests: dictionary persistence and deletion, secure-key entry/removal, key exclusion from snapshots, microphone-test cancellation, backup exclusion, closing the dashboard while retaining the tray engine, and reopening through a second launch. No live Groq inference was used or represented as tested.
- Final observed dashboard launch-to-heading time: 3149 ms, with test-only CDP overhead. Final background working set: 16.61 MiB; see PERFORMANCE.md for methodology and limits.
- The real NSIS package passed guarded per-user **silent install → installed background launch → uninstall** in a new owned directory. Dependency licenses were present and the local profile was retained. The interactive wizard and upgrade path are not certified by that test.
- **Still unverified:** live Groq speech, actual transcription-to-text delivery in Notepad/other apps, clipboard restoration after real insertion, protected-input compatibility, live Command Mode, multi-monitor/DPI scenarios, and a second-machine clean build. The interactive insertion test stopped when Windows refused foreground activation. The release gate remains closed.

The sanitized JSON reports in verification/ are evidence for these specific tests. They are not live provider or full end-to-end acceptance results.

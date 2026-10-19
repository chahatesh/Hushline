# Feature status

Status describes this revision, not a promise of commercial equivalence. **Implemented** means the feature has relevant executed verification; **Partially Implemented** means code exists but behavior is limited or its required real integration remains unverified; **Not Implemented** means absent. The complete acceptance criteria are not yet met. Nothing untested is marked complete.

| Requested feature | Status | Evidence / limit |
| --- | --- | --- |
| Original identity, logo, original source, author attribution | Implemented | Original SVG and source, visible UI screenshots, README author section. |
| Genuine MIT license | Implemented | Complete standard license; Copyright (c) 2026 Chahatesh; SPDX source headers. Third-party terms preserved. |
| Rust/Tauri 2/Solid modular architecture | Implemented | Native compilation and unit tests, production frontend build, actual Tauri IPC smoke test. |
| No Hushline account, subscription, artificial app quota, or telemetry | Implemented | No account/analytics service or app usage gate exists; reviewed source. Groq account terms still apply. |
| Native WASAPI microphone capture | Partially Implemented | CPAL stream, actual-device smoke test provided; see QA for measured execution result. |
| Microphone enumeration and selection | Partially Implemented | Native enumeration plus dashboard selector; duplicate device names are not disambiguated by stable ID. |
| Voice activity detection / sustained silence | Partially Implemented | Energy threshold and duration bounds implemented. Quiet/noisy room validation and semantic speech detection remain incomplete. |
| In-memory WAV, 16 kHz mono, anti-alias resampling | Implemented | Duration, DC preservation, silence, and high-frequency attenuation tests pass; no raw-audio disk writes. |
| Long-session chunking / compression selection | Not Implemented | Sessions capped at 600 seconds, below a conservative 24 MB upload guard. WAV only; no streaming endpoint. |
| Push-to-talk Ctrl+Win | Partially Implemented | Hook routing tests pass. Live microphone → Groq → Notepad hold/release acceptance remains unverified. |
| Hands-free Ctrl+Win+Space | Partially Implemented | Toggle state path, timer, level, silence stop exist; live acceptance pending. |
| Optional double-tap activation | Partially Implemented | 350 ms release arbitration exists; real rapid-gesture test pending. Enabling it intentionally adds up to 350 ms before stop. |
| Escape cancellation and rapid reactivation | Partially Implemented | Cancellation tokens, generations, and idle/modifier Escape routing tested. Real rapid capture/insertion tests pending. |
| Custom keyboard and Mouse4/5 bindings | Partially Implemented | Chord parser and duplicate/conflict tests pass; real extra-mouse-button tests pending. Conflicts with other software cannot all be detected. |
| Copy/paste last, dashboard/scratchpad shortcuts | Partially Implemented | Code paths and UI controls exist; global delivery verification pending. |
| Groq transcription, model/language/vocabulary parameters | Partially Implemented | Documented multipart fields verified using a local HTTP server. No live Groq key was available. |
| Whisper Turbo / accuracy model choice, multilingual recognition | Partially Implemented | Real supported model IDs and language parameter configured; live multilingual accuracy not tested. |
| Timeout, network error, retry, rate-limit, cancellation handling | Partially Implemented | Executed loopback fixtures verify multipart, retry, 401, 429, cancellation, and truncated AI responses. Live provider interruption/rate tests pending. |
| Model discovery and configurable Groq text models | Partially Implemented | Native discovery/cache and dashboard configuration exist; live account model availability unverified. |
| Default fast path without extra AI call | Implemented | Default-settings tests and reviewed branch logic. Literal mode bypasses AI even if an enhanced flag is set. |
| Conservative fillers, repeated articles, weekday self-corrections | Implemented | Regression tests preserve meaningful “had had”, “you know”, identifiers, URLs, and ambiguous revisions. Repairs are intentionally narrow. |
| General false starts, arbitrary self-corrections | Partially Implemented | Optional AI processing can handle them; no reliable local general semantic repair. |
| Local punctuation, paragraphs, bullets | Implemented | English-only explicit commands plus tested automatic notes, ordered cue lists and sentence-based paragraphs. Literal mode and technical identifiers are preserved; broad semantic layout detection remains limited. |
| Numbered lists, dates, times, common-number normalization | Not Implemented | No dedicated reliable local grammar. Optional AI can format text, but is not counted as verified support. |
| Email/technical writing, Literal/Clean/Polished modes | Partially Implemented | Modes/presets exist; literal/local transformations tested, live polished quality and meaning preservation unverified. |
| Native accessibility protection and target/caret identity | Partially Implemented | Fail-closed UIA checks and equality tests exist. Interactive test could not acquire foreground focus in this execution environment. |
| Password and read-only blocking | Partially Implemented | Native checks exist; real protected-control test did not reach its assertions because focus acquisition was refused. |
| Unicode system-wide insertion | Partially Implemented | Compiles; actual delivery into Notepad, browsers, Office, chat apps, and editors remains unverified. |
| Guarded clipboard paste, restoration, newer-copy protection | Partially Implemented | Snapshot/sequence logic implemented. Interactive test blocked before insertion; asynchronous consumer timing and all formats unverified. |
| Direct accessibility text mutation | Not Implemented | UIA inspects targets; delivery defaults to clipboard paste, with opt-in compatibility typing. Whole-value replacement is deliberately avoided. |
| Terminal dictation | Partially Implemented | Transcript recovery available; automatic terminal insertion deliberately blocked. Integrated terminal detection/compatibility remains a limitation. |
| Floating overlay states / waveform / no focus activation | Partially Implemented | A native Win32/GDI overlay passed show/hide/nonactivation and initial-paint tests. Hotkey feedback timing and multi-monitor validation remain unverified. |
| Dark/light/system, high DPI, reduced motion | Partially Implemented | Themes/responsive layout tested in browser. Native DPI/mixed-monitor testing pending; reduced-motion CSS present. |
| Optional quiet sounds | Not Implemented | No sounds shipped. |
| Full nine-section dashboard | Implemented | Browser interaction tests and the actual native IPC smoke test pass; screenshots reviewed. Live dictation acceptance remains open. |
| Home status, shortcut, microphone, model, recent usage | Partially Implemented | Status/shortcut/device/recent usage present. Speech model is selected in Settings rather than displayed as a dedicated Home field. |
| Local history search/copy/recovery/delete/export/timestamps/app | Partially Implemented | SQLite persistence/deletion and browser UI tested. Native IPC and real app attribution unverified. Latest 1,000 records loaded/exported. |
| Private mode / immediate local deletion | Partially Implemented | Setting and backup exclusion tested; current/recording-time privacy guards implemented. Live private dictation test pending. SQLite is not encrypted. |
| Dictionary add/edit/search/import/export, hints and controlled replacements | Implemented | Browser CRUD/persistence and Rust whole-phrase/non-chaining tests pass. Live speech hint accuracy remains unverified. |
| Snippets with multiline, exact triggers, variables, preview | Partially Implemented | Exact-trigger unit tests and multiline UI tests pass; native expansion/confirmation insertion pending. Plain text only; multiline automatic delivery requires clipboard mode. |
| App-specific writing styles and custom presets | Partially Implemented | CRUD/presets verified in UI; live context matching and AI application pending. Rules currently require the style's AI toggle to have an effect. |
| Scratchpad and export | Implemented | Save/reload and word-count tests pass in browser; native storage contract covered by SQLite tests. |
| Statistics | Implemented | Real retained-history calculations, empty-state and zero-usage tests. No fabricated usage, no lifetime claims. |
| Context executable detection | Partially Implemented | Windows process lookup and optional style matching exist; real app compatibility unverified. |
| Browser-domain matching / nearby text permission | Not Implemented | No screenshots, domains, or surrounding text are collected. |
| Command Mode selected text + Groq edit + preview + guarded replace | Partially Implemented | Opt-in selection and preview logic, truncated-response test implemented. End-to-end selection/edit/replacement unverified. |
| Credential Manager secure key storage | Partially Implemented | Native OS-backed storage, dummy-key round-trip and real Tauri key-entry/removal passed. No real Groq authentication tested. |
| Configuration backup/restore | Implemented | UI export/exclusion and transactional invalid-restore rollback tested. Keys/history/scratchpad excluded. |
| Tray, close/reopen, optional startup, single instance | Partially Implemented | Actual desktop close-to-tray and reopen passed, as did installed background launch/uninstall. Startup opt-in and upgrades still need manual checks. |
| Windows installer and portable artifact | Partially Implemented | Packaging configuration/scripts exist; consult QA for generated artifact/build result. Silent install/background launch/uninstall passed; interactive wizard and upgrades pending. |
| Automatic signed updater / update checks | Not Implemented | Manual trusted installer updates only. |
| Diagnostics / benchmark report | Partially Implemented | Local test reports, history timing, and process-tree measurement script. Actual measured results and gaps are in PERFORMANCE.md. No in-app diagnostic bundle or update service. |
| Public repository / downloadable GitHub Release | Partially Implemented | Public repository created at https://github.com/chahatesh/Hushline. Source publication and hosted checks are tracked in QA; no unverified GitHub Release is published. |
| CI/CD workflows | Partially Implemented | Windows lint/test/build/package and gated draft-release workflows written; no hosted run without a published repository. |
| Broad real Windows application compatibility | Not Implemented | No universal compatibility claim. Required live Notepad/browser/Office/editor/chat matrix remains open. |
| Performance engineering targets | Partially Implemented | Event-driven architecture, bounded buffers, reused HTTP client, opt-in AI, released dashboard. Final fresh-background run measured 16.61 MiB working set, 3.61 MiB private, and rounded 0% CPU. No live speech latency claim. |

The decisive missing evidence is an uninterrupted real user session: install → enter a valid Groq key → choose a microphone → focus Notepad → hold/speak/release → correct text in the original field, followed by the same workflow's cancellation, focus-change, private-mode, clipboard, and command-edit checks.

## 0.2 additions

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Offline Whisper speech | Partially Implemented | Verified Tiny model download and real ~11-second WAV transcription on the CPU (2,751 ms in one run). Base weights are pinned but not accuracy-tested. Live microphone → offline → application acceptance remains open. |
| Optional local Ollama editing | Partially Implemented | Loopback-only client, local metadata check, cloud-model rejection, cancellation and response checks implemented. An actual downloaded Ollama model was not available for end-to-end quality testing. |
| Automatic notes/list/paragraph layout | Implemented | Regression tests verify explicit note cues, ordered first/second/third lists, paragraph grouping, protected identifiers, and Literal preservation. English rules only. |
| One-action paste default and upgrade migration | Implemented | Old preferences migrate to clipboard once without changing custom shortcuts. Actual paste/restoration compatibility remains separately unverified. |
| Shortcuts: key-up pairing and prefix release | Partially Implemented | Tests cover hands-free/command/paste chord releases, repeated keydown, 100 rapid presses, idle Escape, and plain Space not opening the app. Live hardware/system-reserved chord interactions require user validation. |
| Hold-to-talk through silence | Implemented | Silence-stop predicate is applied only to hands-free sessions and tested; maximum duration still applies to every session. |
| Compact resting pill and docking | Partially Implemented | Native bottom/left/right layouts, translucent idle, opaque active, mic/wave, small expansion and reduced-motion handling implemented. Native dimensions, focus, and opacity checks are recorded in QA; mixed-DPI monitors still require manual testing. |
| Shortcut recorder and dedicated-key preset | Partially Implemented | Browser tests exercise chord capture and persistence; native shortcuts suspend with a bounded capture timeout. Hardware-reserved combinations remain outside universal control. |

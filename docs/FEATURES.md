# Feature status

Status describes 0.5.2, not a promise of commercial equivalence. Versioned sections preserve earlier evidence; current behavior and remaining acceptance checks are summarized below. **Implemented** means the feature has relevant executed verification; **Partially Implemented** means code exists but behavior is limited or its required real integration remains unverified; **Not Implemented** means absent. The complete acceptance criteria are not yet met. Nothing untested is marked complete.

| Requested feature | Status | Evidence / limit |
| --- | --- | --- |
| Original identity, logo, original source, author attribution | Implemented | Original SVG and source, visible UI screenshots, README author section. |
| Genuine MIT license | Implemented | Complete standard license; Copyright (c) 2026 Chahatesh; SPDX source headers. Third-party terms preserved. |
| Rust/Tauri 2/Solid modular architecture | Implemented | Native compilation and unit tests, production frontend build, actual Tauri IPC smoke test. |
| No Hush account, subscription, artificial app quota, or telemetry | Implemented | No account/analytics service or app usage gate exists; reviewed source. Groq account terms still apply. |
| Native WASAPI microphone capture | Partially Implemented | Stopped native-client preparation, actual frames/cancellation and normal Stop passed. More devices and full microphone-to-target compatibility remain unverified. |
| Microphone enumeration and selection | Partially Implemented | Native enumeration plus dashboard selector; duplicate device names are not disambiguated by stable ID. |
| Voice activity detection / sustained silence | Partially Implemented | Energy threshold and duration bounds implemented. Quiet/noisy room validation and semantic speech detection remain incomplete. |
| In-memory WAV, 16 kHz mono, anti-alias resampling | Implemented | Duration, DC preservation, silence, and high-frequency attenuation tests pass; no plaintext audio disk writes; failed-audio DPAPI recovery is documented below. |
| Long-session chunking / compression selection | Not Implemented | Sessions capped at 600 seconds, below a conservative 24 MB upload guard. WAV only; no streaming endpoint. |
| Push-to-talk Ctrl+Win | Partially Implemented | Hook routing tests pass. Live microphone → Groq → Notepad hold/release acceptance remains unverified. |
| Hands-free Ctrl+Win+Space | Partially Implemented | Toggle state path, timer, level, silence stop exist; live acceptance pending. |
| Optional double-tap activation | Partially Implemented | 350 ms release arbitration exists; real rapid-gesture test pending. Enabling it intentionally adds up to 350 ms before stop. |
| Escape cancellation and rapid reactivation | Partially Implemented | Cancellation tokens, generations, and idle/modifier Escape routing tested. Real rapid capture/insertion tests pending. |
| Custom keyboard and Mouse4/5 bindings | Partially Implemented | Chord parser and duplicate/conflict tests pass; real extra-mouse-button tests pending. Conflicts with other software cannot all be detected. |
| Copy/paste last, dashboard/scratchpad shortcuts | Partially Implemented | Code paths and UI controls exist; global delivery verification pending. |
| Groq transcription, model/language/vocabulary parameters | Partially Implemented | Documented multipart fixtures pass. The 0.3.1 saved-key public speech fixture passed through Groq Turbo in 1,342 ms; live microphone-to-target acceptance remains open. |
| Whisper Turbo / accuracy model choice, multilingual recognition | Partially Implemented | Real supported model IDs and language parameter configured; live multilingual accuracy not tested. |
| Timeout, network error, retry, rate-limit, cancellation handling | Partially Implemented | Executed loopback fixtures verify multipart, retry, 401, 429, cancellation, and truncated AI responses. Live provider interruption/rate tests pending. |
| Model discovery and configurable Groq text models | Partially Implemented | Native discovery/cache and dashboard configuration exist; live account model availability unverified. |
| Default fast path without extra AI call | Implemented | Default-settings tests and reviewed branch logic. Literal mode bypasses AI even if an enhanced flag is set. |
| Conservative fillers, repeated articles, weekday self-corrections | Implemented | Regression tests preserve meaningful “had had”, “you know”, identifiers, URLs, and ambiguous revisions. Repairs are intentionally narrow. |
| General false starts, arbitrary self-corrections | Partially Implemented | Explicit corrections have stronger local handling and corrected-meaning AI validation in 0.4.0. Ambiguous wording is preserved; arbitrary semantic repair and universal accuracy are not claimed. |
| Local punctuation, paragraphs, bullets | Implemented | English-only explicit commands plus tested automatic notes, ordered cue lists and sentence-based paragraphs. Literal mode and technical identifiers are preserved; broad semantic layout detection remains limited. |
| Explicit numbered lists | Implemented | Tested spoken numbered-list commands across Casual/Standard/Formal, sequential numeric items through 15, multi-sentence items and surrounding prose. Literal mode preserves recognition. Ambiguous cues remain conservative. |
| General dates, times and number normalization | Partially Implemented | No universal normalization engine; optional AI output is not counted as verified general grammar. |
| Casual/Standard/Formal writing and advanced exact words | Partially Implemented | Independent style/AI choices and migration pass UI/unit checks. Local rules are English and conservative; stronger Formal rewriting requires AI. Full live writing quality/meaning preservation remains unverified. |
| Dictation through text-box layout changes | Partially Implemented | Screen-coordinate failure reproduced in a real Chrome field before the fix; new collapsed-selection and same-field/input regressions pass. All six real Chrome stable/reflow/changed-field cases pass after the fix; full speech-to-target acceptance remains open. |
| Native accessibility protection and target/caret identity | Partially Implemented | Fail-closed UIA checks and equality tests exist. Interactive test could not acquire foreground focus in this execution environment. |
| Password and read-only blocking | Partially Implemented | Native checks exist; the foreign-process Edit fixture verifies password/read-only rejection, while full live application compatibility remains pending. |
| Unicode system-wide insertion | Partially Implemented | Compiles; actual delivery into Notepad, browsers, Office, chat apps, and editors remains unverified. |
| Guarded clipboard paste, restoration, newer-copy protection | Partially Implemented | Native bitmap/metafile ownership tests and actual image clipboard round-trip pass. Earlier owned-field insertion/restoration passed. Browser/custom consumers, delayed rendering and all formats remain unverified; unsupported formats refuse before mutation. |
| One-operation native control insertion | Partially Implemented | Real foreign-process Unicode Edit fixture passes multiline insertion, selection replacement, Undo, cancellation and protection. Native indices survive visual reflow. Actual Notepad/RichEdit application and microphone-to-target acceptance remain pending. |
| Recovery buttons and feedback ownership | Partially Implemented | Paste here/Copy text cards, scaled hit tests, preview exclusion, stale feedback/timer regressions verified. Complete live shortcut-to-recovered-paste workflow remains pending. |
| Approved whole-field native/accessibility replacement | Partially Implemented | Improve prompt can replace one unchanged original field after edited-preview approval. Complete field/protection/identity checks apply; real target compatibility remains pending. Ordinary dictation uses smart native insertion or guarded clipboard fallback, with opt-in typing. |
| Terminal dictation | Partially Implemented | Transcript recovery available; automatic terminal insertion deliberately blocked. Integrated terminal detection/compatibility remains a limitation. |
| Floating overlay states / waveform / no focus activation | Partially Implemented | A native Win32/GDI overlay passed show/hide/nonactivation and initial-paint tests. Hotkey feedback timing and multi-monitor validation remain unverified. |
| Dark/light/system, high DPI, reduced motion | Partially Implemented | Themes/responsive layout tested in browser. Native DPI/mixed-monitor testing pending; reduced-motion CSS present. |
| Optional quiet recording-ready sound | Partially Implemented | Original quiet cue and on/off setting added. Trigger is first microphone frames, after preparation; audible live/device acceptance remains pending. |
| Full nine-section dashboard | Implemented | Browser interaction tests and the actual native IPC smoke test pass; screenshots reviewed. Live dictation acceptance remains open. |
| Home status, shortcut, microphone, model, recent usage | Partially Implemented | Status, shortcut, device, selected speech model and recent usage are displayed. Live status accuracy during dictation remains pending. |
| Local history search/copy/recovery/delete/export/timestamps/app | Partially Implemented | SQLite persistence/deletion and browser UI tested. Native IPC and real app attribution unverified. Latest 1,000 records loaded/exported. |
| Private mode / immediate local deletion | Partially Implemented | Setting and backup exclusion tested; current/recording-time privacy guards implemented. Live private dictation test pending. SQLite is not encrypted. |
| Dictionary add/edit/search/import/export, hints and controlled replacements | Implemented | Browser CRUD/persistence and Rust whole-phrase/non-chaining tests pass. Live speech hint accuracy remains unverified. |
| Snippets with multiline, exact triggers, variables, preview | Partially Implemented | Exact-trigger unit tests and multiline UI tests pass; native expansion/confirmation insertion pending. Plain text only; multiline automatic delivery requires clipboard mode. |
| App-specific writing styles and custom presets | Partially Implemented | CRUD/presets verified in UI; live context matching and AI application pending. Rules currently require the style's AI toggle to have an effect. |
| Scratchpad and export | Implemented | Save/reload and word-count tests pass in browser; native storage contract covered by SQLite tests. |
| Statistics | Implemented | Real retained-history calculations, empty-state and zero-usage tests. No fabricated usage, no lifetime claims. |
| Context executable detection | Partially Implemented | Windows process lookup and optional style matching exist; real app compatibility unverified. |
| Browser-domain matching / automatic nearby text | Not Implemented | No screenshots, domains, or automatic surrounding text are collected. Explicit selected-text Command Mode and whole-field Improve prompt are separate authorized actions. |
| Command Mode selected text + Groq edit + preview + guarded replace | Partially Implemented | Opt-in selection and preview logic, truncated-response test implemented. End-to-end selection/edit/replacement unverified. |
| Credential Manager secure key storage | Partially Implemented | Native OS-backed storage, earlier dummy-key round-trip and Tauri key-entry/removal passed. Saved real key authenticated the public Groq fixture and was preserved. Independent upgrade/manual acceptance remains open. |
| Configuration backup/restore | Implemented | UI export/exclusion and transactional invalid-restore rollback tested. Keys/history/scratchpad excluded. |
| Tray, close/reopen, optional startup, single instance | Partially Implemented | Actual desktop close-to-tray and reopen passed, as did installed background launch/uninstall. Startup opt-in and upgrades still need manual checks. |
| Windows installer and portable artifact | Partially Implemented | Packaging configuration/scripts exist; consult QA for generated artifact/build result. Silent install/background launch/uninstall passed; interactive wizard and upgrades pending. |
| Automatic signed updater / update checks | Not Implemented | Manual trusted installer updates only. |
| Diagnostics / benchmark report | Partially Implemented | Bounded memory-only stage timings/outcomes can be explicitly exported; no audio/text/key data. Historical and current measured results are in PERFORMANCE.md. No automatic upload or update service. |
| Public repository / downloadable GitHub Release | Partially Implemented | Public repository created at https://github.com/chahatesh/Hushline. Source publication and hosted checks are tracked in QA; no unverified GitHub Release is published. |
| CI/CD workflows | Partially Implemented | Hosted 0.3.2 lint/test/build, real offline fixture, installer and portable artifact generation passed. Run/commit evidence is in QA. Release automation remains gated by incomplete manual acceptance. |
| Broad real Windows application compatibility | Not Implemented | No universal compatibility claim. Required live Notepad/browser/Office/editor/chat matrix remains open. |
| Performance engineering targets | Partially Implemented | Event-driven architecture, bounded buffers, reused HTTP client, opt-in AI, released dashboard. The 0.3.2 fresh-background run measured 18.93 MiB working set, 4.08 MiB private, rounded 0% CPU. Measurement limits are in PERFORMANCE.md. No live speech latency claim. |

The decisive missing evidence is an uninterrupted real user session: install → enter a valid Groq key → choose a microphone → focus Notepad → hold/speak/release → correct text in the original field, followed by the same workflow's cancellation, focus-change, private-mode, clipboard, and command-edit checks.

## 0.2 additions

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Offline Whisper speech | Partially Implemented | Verified Tiny model download and real ~11-second WAV transcription on the CPU (2,751 ms in one run). Base weights are pinned but not accuracy-tested. Live microphone → offline → application acceptance remains open. |
| Optional local Ollama editing | Partially Implemented | Loopback-only client, local metadata check, cloud-model rejection, cancellation and response checks implemented. A downloaded qwen3:0.6b passed three isolated synthetic requests; latency and incomplete formatting are recorded in LOCAL_MODEL.md. Production local-edit acceptance remains open. |
| Automatic notes/list/paragraph layout | Implemented | Regression tests verify explicit note cues, ordered first/second/third lists, paragraph grouping, protected identifiers, and Literal preservation. English rules only. |
| One-action paste default and upgrade migration | Implemented | Old preferences migrate to clipboard once without changing custom shortcuts. Actual paste/restoration compatibility remains separately unverified. |
| Shortcuts: key-up pairing and prefix release | Partially Implemented | Tests cover hands-free/command/paste chord releases, repeated keydown, 100 rapid presses, idle Escape, and plain Space not opening the app. Live hardware/system-reserved chord interactions require user validation. |
| Hold-to-talk through silence | Implemented | Silence-stop predicate is applied only to hands-free sessions and tested; maximum duration still applies to every session. |
| Compact resting pill and docking | Partially Implemented | Native bottom/left/right layouts, translucent idle, opaque active, waveform without a mic icon, small expansion and reduced-motion handling implemented. Native dimensions, focus, and opacity checks are recorded in QA; mixed-DPI monitors still require manual testing. |
| Shortcut recorder and dedicated-key preset | Partially Implemented | Browser tests exercise chord capture and persistence; native shortcuts suspend with a bounded capture timeout. Hardware-reserved combinations remain outside universal control. |

## 0.2.1 refinements

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Stopped microphone preparation | Implemented | Real native worker tested three starts/cancellations: Ready 21/5/4 ms, first frames 21/68/61 ms after preparation. No idle audio reads; an off switch releases preparation. Cold setup and other devices vary. |
| Smaller smooth-edged pill without mic symbol | Partially Implemented | Native per-pixel rendering, fractional alpha, full active opacity, geometry and nonactivation passed; PNGs inspected. Mixed-DPI/multi-monitor/manual user acceptance remains open. |
| Natural list introductions and paragraphs within line breaks | Implemented | Reproduced failures now pass with preserved ordinary-prose word sequence. Clear English list cues and punctuation boundaries; no claim of universal semantic understanding. |
| Repeated words and restart fragments | Implemented | Regression tests cover multiple adjacent function words, I-stutters, comma-separated hesitation and a narrow set of exact repeated restart phrases. Meaningful emphasis and grammatical repeats are preserved. |
| Ten-minute default and opt-in silence stop | Partially Implemented | Bounds/migration and no-silence-stop tests pass. A 125-second synthetic WAV preserves its final sample. Uninterrupted long live-speech acceptance remains open. |
| Clipboard completion before restoration delay | Partially Implemented | Dispatch acknowledgement and delayed format/sequence restoration separated; unsupported formats remain refused. Real target insertion/restoration is still blocked by the environment's foreground restriction. |
| Conservative automatic AI-edit content guard | Partially Implemented | Tests reject major shortening and changed identifiers/numbers; this does not prove preservation of every fact or nuance. Command Mode retains explicit edit authority. |
| Low reasoning / local context budget | Implemented | Documented GPT-OSS-only request field tested against HTTP fixtures; unrelated models omit it. Local context bounds/truncation refusal tested; real local-model trial uses thinking off. Quality remains separately limited. |
| Small local-model feasibility trial | Implemented | Downloaded 523 MB model, measured 3–15 s synthetic edits and substantial active working set, stopped the isolated service. Not enabled as an automatic dictation dependency. |
## 0.3.0 refinements

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Hush product name | Implemented | UI, tray, window, installer and About use Hush. Stable identifier, executable and credential/data names retain upgrade compatibility. |
| Installer coexistence with an older Hush app | Implemented | A unique bundle-ID registration and Hush Dictation shortcuts passed real silent install/uninstall while retaining the pre-existing Hush registration and shortcut hashes. Vendored upstream template attribution/license retained. |
| Error and recovery card above the pill | Partially Implemented | Real Win32 rendering/nonactivation passed at bottom/left/right docks. Synthetic native pixels inspected. Mixed-DPI monitor verification remains open. |
| Always / Sometimes / Never AI polish | Implemented | Policy/migration regression and browser persistence tests pass. Real Groq synthetic requests pass for Always and Sometimes; Never skips the edit call. Literal words and snippets bypass automatic AI editing. |
| Robust mixed prose/list layout | Partially Implemented | Tests cover English introductions, prose before an enumerated list and existing Markdown bullets. Ambiguous topic/list structure relies on optional AI; no universal semantic-accuracy claim. |
| Sentence-aware terminal periods | Partially Implemented | Clear English fragments lose a terminal dot; recognized complete sentences/identifiers/literal text retain punctuation. Unknown local constructions retain their punctuation. AI is instructed to handle other fragments. |
| Reported intermittent red first-use state | Partially Implemented | Cold first-start plus four normal-stop captures passed, and the configured mic completed the packaged test. The user's exact intermittent failure was not reproduced conclusively. Error details are now visible instead of only a red symbol. |

## 0.3.1 refinements

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Unified upward expansion and stronger palette | Partially Implemented | Real native layered-window painting, fixed lower anchors, focus preservation, opaque active body and inspected pixels passed. Browser light/dark controls and upward preview passed. Actual mixed-DPI/multi-monitor and manual visual acceptance remain open. |
| Faster voice feedback and reused rendering resources | Implemented | Active capture interval is 20 ms; native update messages are coalesced, dashboard metering is capped at 10 Hz, and no idle timer was added. A real 5-second synthetic 50 Hz native workload reused its surface without reallocations. Full hotkey-to-screen latency remains unmeasured. |
| Complete distinct dictionary hints | Implemented | Executed regressions cover whole names, whitespace, duplicates, oversized replacements and later smaller terms within the 800-character budget. General speech word-error improvement is not measured. |
| Audio-gap recovery and review guard | Partially Implemented | A real device gap was reproduced. A forced 300 ms reader overrun now preserves later frames and marks the recording. Engine regression blocks automatic insertion, and interrupted captures skip snippets/AI polishing. Real normal Stop and cancellation passed. Full interrupted speech/transcription/manual review acceptance remains open; missing audio is not recoverable. |

## 0.3.2 corrections

| Feature | Status | Evidence / limits |
| --- | --- | --- |
| Brief-hiccup dictation and substantial-loss review | Partially Implemented | Three real captures plus the packaged UI mic check passed without a review warning. Policy regressions and a 500 ms forced loss pass the substantial-loss guard. Lost speech is not reconstructed and broad live target acceptance remains open. |
| Cursor, timed collapse and centered waveform | Partially Implemented | Native class/client cursor, 2.5-second contraction, centered bottom/side waveform, same lower anchor and unchanged listening/focus checks pass. Actual physical mixed-monitor acceptance remains open. |

### 0.3.3 timeout and pill update

| Feature | Status | Evidence / limit |
| --- | --- | --- |
| Pill label removal, timer beside × | Partially Implemented | Native and browser layout checks run for bottom/side docks; physical mixed-DPI acceptance remains open. |
| Five-second speech deadline, exact error | Partially Implemented | Cancellation and late-result regression; delayed loopback HTTP fixture exercises the real speech client. Real slow Groq/Notepad acceptance remains open. |
| Encrypted failed-audio recovery | Partially Implemented | Windows DPAPI/SQLite restart, corruption, retention, expiry and memory-only private tests. Home exposes Retry/Delete; complete live microphone → timeout → retry flow awaits manual acceptance. |
| Retry avoids old insertion target | Partially Implemented | New generation has no target/selection; recovered text is copy-only. Live target application acceptance remains open. |
| Bounded optional AI editing | Partially Implemented | Five-second cleanup limit falls back locally; ten-second command-edit limit fails safely. Live provider timeout acceptance remains open. |

## 0.4.0 reliable recording and writing

| Feature | Status | Evidence / limit |
| --- | --- | --- |
| Configurable startup pause and first-frame readiness | Partially Implemented | Default 300 ms, range 0–1,500; dark preparation and five-second microphone-start bound. Abandoned holds must not start later. Live hardware/release acceptance remains pending. |
| Recording-ready cue and recording-only bright waveform | Partially Implemented | Sound preference and state presentation exist; browser checks verify preference persistence. Audible cue timing and real microphone-driven display remain subject to native/live acceptance. |
| Timer left, × right; side timer above | Partially Implemented | Browser geometry checks pass, including centered side preview and reduced motion. Native physical mixed-DPI/manual acceptance remains open. |
| Retained pending target inspection and bounded delivery | Partially Implemented | Processing carries the original target inspection; insertion remains guarded with a two-second delivery bound. Delayed inspection, dashboard/copy invalidation, cancellation, and no-late-paste behavior need the recorded native/live checks in QA. |
| Casual/Standard/Formal selection independent of AI policy | Implemented | Executed UI/unit checks verify style choice, Always/Sometimes/Never independence, exact-word override, schema-5 migration, and preservation of existing settings. Stronger Formal AI quality is separately partial. |
| Explicit correction chains and corrected-meaning AI guard | Partially Implemented | Local supported corrections and protected-content validation are implemented with regression cases. Ambiguous wording remains conservative; full live speech/model-quality acceptance remains open. |
| List introductions, numbered cues, multi-sentence items, mixed prose | Partially Implemented | Local parser has targeted regression coverage and keeps ordinary conjunctions conservative. Universal semantic layout and live recognizer/model accuracy are not claimed. |
| Mode-specific cloud/local AI instructions and local fallback | Partially Implemented | Instructions request concise, meaning-preserving output and distinguish Casual/Standard/Formal. Five-second automatic-cleanup fallback remains. Actual quality across languages/models needs further evaluation. |
| Shortcut-only Improve prompt: whole focused field | Partially Implemented | Default Ctrl+Win+P, no microphone, complete editable-field read up to 32,000 UTF-8 bytes with no truncation. Protected/read-only/terminal/unsupported inputs are refused. Live application support and end-to-end acceptance remain pending. |
| Clarify/Expand, editable preview, approved replacement | Partially Implemented | Action preference and shortcut persistence pass browser checks. Twenty-second AI deadline; output up to 100,000 UTF-8 bytes. Apply rechecks identity/full original text; changed targets keep a copyable proposal. Native/live replacement and provider quality remain pending. |
| Transient prompt privacy and bounded stage metadata | Partially Implemented | Prompt source/output are excluded from automatic history, backup, and diagnostics. Stage metadata is memory-only, at most 200 events across 50 generations, with explicit export. Full live privacy acceptance remains open. |

The following section records the earlier 0.4.2 update. Historical native captures, provider fixtures, and measurements remain evidence for their recorded builds only; consult QA and PERFORMANCE for current executed checks. No new live feature is declared complete based only on browser presentation or a mocked provider.

| 0.4.2 reliability update | Status | Evidence / limit |
| --- | --- | --- |
| Stop-click preserves original field authorization | Partially Implemented | Mouse-hook regression and native overlay hit-test checks cover the corrected path; uninterrupted live speech to a target still needs acceptance. Other clicks and scrolling retain revocation. |
| Short-tap recognition and second hold/release stop | Partially Implemented | Release-intent regressions cover taps, long holds and hands-free finish. Real key/microphone-to-provider acceptance remains pending. |
| Fixed inspection/paste failure reason codes | Implemented | Bounded in-memory diagnostics accept only fixed codes; arbitrary error payloads are excluded by regression tests. |
| Windows control insertion and clipboard protection | Partially Implemented | Owned Win32 integration passed actual Unicode/paste delivery, clipboard restoration, cancellation, changed-target and password checks. Browser and broad application acceptance remain pending. |

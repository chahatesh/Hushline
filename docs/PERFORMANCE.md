# Performance report

No end-to-end transcription latency, under-100 MB idle footprint, or sub-100 ms recording-feedback claim is made without a measurement.

## Environment and methodology

Local implementation/testing on Windows NT 10.0.26300.0, x64, Intel Core i5-1235U, 12 logical processors, 7.7 GiB reported physical memory. The Rust compiler reports stable 1.99.1/MSVC; Node 22.14.0. Microsoft compiler/SDK files were extracted into the ignored workspace-local tools directory. The build was running during early smoke tests; those samples are not a quiet-machine benchmark. CPU/RAM and version information are recorded by `scripts/benchmark.ps1` when an optimized executable is available.

## Initial 0.1.0 development observations

These are historical observations from initial implementation. Current-revision measurements and their limits appear at the end of this report.

| Measurement | Result | Scope / limitation |
| --- | --- | --- |
| Microphone activation to stream-ready | **2,869 ms** | One actual default-device debug smoke run, under compilation load. Includes device/configuration/stream startup; not a warm-session average or overlay feedback measurement. This is too slow for the intended daily-use experience. |
| Native microphone frames + cancellation | Passed | Actual device delivered frames and was cancelled. No audio upload or raw-audio file. |
| Credential Manager round-trip | Passed in 0.11 s test duration | Dummy credential saved/read/deleted; does not measure real Groq authentication or per-command UI latency. |
| Backend unit/HTTP suite | 25 passed in 0.18 s test execution | Debug test binary after compilation; loopback fixtures, not cloud performance. |
| Frontend production payload | About 79.6 KB JS / 25.5 KB CSS before gzip; 27.1 KB / 6.0 KB gzip | Initial production build; final build may vary slightly. Not an installed-size or RAM measurement. |
| Initial browser-overlay idle RAM / CPU | 350.03 MiB summed working set, 148.36 MiB private; 0.098% CPU | Initial 20-sample run before the native-overlay change. Included seven parent/WebView2 processes; summed working sets can double-count shared pages. This missed the memory target and triggered the architectural change. |
| Startup to usable dashboard | 18,990 ms in first desktop smoke run | Initial app under compilation load, before the native-overlay change; includes test-only CDP setup/connection overhead. Not a pure cold-start measurement or percentile. |
| Visible hotkey feedback | Unverified | Requires synchronized hook/UI measurement on an interactive desktop. |
| Groq speech round-trip / end-to-end insertion | Unverified | No live Groq key/spoken acceptance sample; native foreground test was refused by Windows. |

## Engineering choices in this revision

- Rust owns the critical path; the settings UI does not mediate recording or HTTP calls.
- Keyboard/mouse and focus notifications are event-driven. No idle microphone stream or continuous animation loop runs.
- The dashboard webview is created on demand and destroyed on close. Background startup creates no webviews. A hidden native GDI popup provides the overlay, without WebView2.
- Audio capture buffers are bounded/preallocated; native packets are downmixed on the audio thread. Since 0.3.1 the native meter targets 50 Hz while recording and dashboard messages are limited to 10 Hz.
- Microphone startup is explicitly labeled “Starting microphone” until the stream reports ready; early cancellation is checked before activation.
- A 32-tap, 256-phase windowed-sinc table avoids per-output-sample sine/cosine work during resampling. Signal-quality regression tests cover duration, DC preservation, and rejection above the output Nyquist frequency. No unsupported speedup ratio is claimed.
- Short recordings use 16 kHz mono 16-bit WAV in memory, avoiding encoding subprocesses and temporary audio files. Maximum length is bounded; no chunking is implemented.
- A reused native HTTP client amortizes connections. Simple dictation skips an extra text-model call. Model discovery is cached for optional text editing.
- The native overlay has no timer while hidden. Its short fade and processing timer run only while active, and Windows reduced-motion preferences disable those animations.

## Reproduce

Quit Hushline first, use a release executable, keep the machine idle, and run:

```powershell
powershell -NoProfile -File scripts/benchmark.ps1
node tests/native/desktop-smoke.mjs
```

The first script launches only the newly built app, warms up for five seconds, samples the parent and its existing WebView2 descendants for twenty seconds, and records working set, private bytes, and CPU as a percentage of all logical processors in `artifacts/performance.json`. It then stops the process it launched. Sampling itself and other applications introduce overhead; repeat at least five times on a quiet machine before publishing a performance claim. The second script uses a test-only loopback debugging port to verify the actual Tauri/native bridge; the shipping app does not enable that port.

For meaningful latency results, separately record cold/warm microphone activation, keypress-to-visible-overlay feedback, audio duration, WAV encoding, upload/transcription, optional AI cleanup, insertion, and total release-to-text time over at least 30 real utterances. Report model, microphone, audio lengths, language, network RTT, median and 95th percentile, failures, and hardware. Compare the fast path and AI-enabled path separately. This revision has not completed that study.

## Remaining performance work

Extend device/driver coverage for stopped-client preparation and packet-gap recovery. Verify complete speech-to-target timing and physical mixed-DPI overlay behavior. Fresh-background measurements meet the memory target on this machine; dashboard/inference sessions and broader hardware need separate measurements.

## 0.1.0 baseline native-overlay measurements

Measured 2026-10-18T17:04:33.8343133Z on the hardware above, using the packaged release executable (SHA-256 `AFA6DB924AED82FFFAD06426080FE194E3FDD347AAD93C0A8DFFDC3B7FBB3DE9`). This run had **one Hushline process and no WebView2 children**: background tray startup creates no webviews. The dashboard is created only when opened.

| Final measurement | Result |
| --- | --- |
| Mean / peak working set | **16.61 / 16.61 MiB** |
| Mean private memory | **3.61 MiB** |
| CPU across 12 logical processors | **0.000% at this measurement resolution** |
| Sample duration after 5-second warmup | 21.62 s, 20 samples |
| Dashboard launch-to-heading observation | **3.149 s**, including test-only CDP connection overhead |
| Actual desktop lifecycle | Close to tray and reopen passed |
| Installer | Per-user silent install, background launch, dependency notices, uninstall and retained profile passed |

The background-memory target was met in this run. This is a single fresh-background observation, not a guarantee for other hardware or a measurement with the dashboard open or following live dictation. The dashboard uses WebView2 while open and consumes additional memory. Working-set sums can double-count shared pages when multiple processes exist; the final background run had only one process. Rounded zero CPU is not a claim that no CPU instruction ever executes.

Full sanitized measurement records: [performance](verification/performance.json), [real desktop smoke](verification/desktop-smoke.json), [installer smoke](verification/installer-test.json). Live Groq transcription-to-insertion latency remains unmeasured.

## 0.2.0 measurements — 2026-10-19

- A real CPU-only Tiny Whisper run processed the upstream approximately 11-second JFK WAV in **2,751 ms**, including hidden process startup/model load and local transcript parsing. The model was newly downloaded and its pages may be in the OS cache. This is one observation, not a percentile, and excludes microphone capture, user speaking time, cloud inference and paste. The fixture and checksum are recorded in `verification/update-0.2.0.json`.
- Native microphone setup reached stream-ready in **2,221 ms** in one debug smoke run during release compilation. It delivered frames and cancelled without an upload. This remains a startup limitation; no sub-100 ms microphone activation claim is made.
- The voice bar's active bottom geometry is **216×40 logical pixels** and resting geometry **72×24**; the side bar is **40×156 active** and **24×64 resting**. Native tests verified opacity and focus preservation. No idle animation timer runs.
- The web production build now transforms **48 modules**, compared with 1,765 before direct icon imports. A subsequent Vite phase took 3.08 seconds here; this is build tooling improvement, not a claimed user-facing runtime speedup. The current payload is approximately **92.82 KB JavaScript / 31.69 KB CSS**, or **31.07 KB / 7.18 KB gzip**.
- Paste is now the default and sends four key events for one paste operation, rather than two events per UTF-16 code unit. Two redundant UIA inspections were removed. These are code-path improvements; actual insertion speed/compatibility is still unverified because Windows denied focus to the owned insertion harness. The 600-ms clipboard restoration wait remains conservative.

The earlier 16.61 MiB result is a **0.1.0 baseline**, not a claim about this revision. Offline model inference and an external Ollama service have additional memory/CPU requirements while active; the fresh background measurement excludes resident models and does not include an unrelated local Ollama service.

A later instrumented real-device debug run reached stream-ready in **2,006 ms**: device selection at 12 ms, configuration at 13 ms, and stream creation completed at 2,005 ms. The delay is concentrated in Windows audio-stream creation. Frames and cancellation passed. These are individual observations, not a statistical latency study; the microphone is not kept open while idle to hide this cost.

### Packaged 0.2.0 background run

Measured 2026-10-19T23:54:42.6694110Z, after the build finished, using executable SHA-256 `7E3B5E5D94C25E3E7260F730A904CC8F75FD99444FE282BA2E548975E543A19B`. The resting native dock was visible; no dashboard webview or offline model was resident. Five seconds of warmup preceded 20 samples over 22.29 seconds.

| Measurement | Result |
| --- | --- |
| Mean / peak working set | **15.94 / 15.94 MiB** |
| Mean private memory | **3.46 MiB** |
| CPU across all logical processors | **0.000% at the sampling resolution** |
| Dashboard launch-to-heading observation | **3896 ms**, including test-only CDP overhead |

This is one fresh-background run on the hardware above, not a guarantee or a measurement during inference. Model decoding and the open dashboard add resource use. Rounded zero CPU does not mean no CPU instructions execute. The earlier dashboard smoke observation was 4,770 ms; neither is a pure cold-start percentile. The live microphone-to-target path remains unmeasured. See [background record](verification/performance-0.2.0.json).

## 0.2.1 recording and provider measurements

Same documented hardware, 2026-10-19. A stopped native WASAPI client prepared once in 1,179 ms; direct starts subsequently produced frames in 25/8/7 ms. The final audio worker reported **21/5/4 ms to Ready** and **21/68/61 ms to first received frames** for three starts. It checks default-device identity and runs capture/metering at up to 20 Hz while active. Idle preparation is stopped: there is no sample read loop, resident speech model or idle audio timer. The initial preparation cost is not eliminated; settings/device changes or an early press can still incur it. These are debug observations and not a 95th-percentile/hardware-wide guarantee.

One actual Groq Turbo request processed the approximately 11-second public JFK fixture in **2,542 ms**, cold HTTP client, including DNS/TLS/upload/server work/parsing. This excludes microphone initialization, speaking time, local formatting and target insertion. The configured key was used without printing or modifying it; private speech was not uploaded. A true release-to-text latency distribution remains unmeasured.

Paste completion now acknowledges dispatch before the conservative 600-ms restoration job finishes. Restoration still retains its owner and sequence-number guards. This removes a fixed wait from completion UI; receiving-application timing is not verified because Windows refused the test harness's foreground acquisition.

The small local text-model trial added 3,403–14,616 ms and substantial active memory; it is kept optional. See LOCAL_MODEL.md for cache/load/quality and memory-pressure limits. The final packaged background measurement is recorded below; 15.94 MiB in the prior section is the 0.2.0 baseline.

### Packaged 0.2.1 background run

Measured 2026-10-20T01:00:32.4484818Z (2026-10-19 evening in America/New_York), after compiler/test jobs finished. Binary SHA-256: `9329FA28ADB8CA71F6D015C569FADD58979F455C175D9374B63CE7CE35CC9A69`. The resting pill and stopped microphone preparation were enabled; no webview or local inference model was resident. Five seconds of warmup preceded 20 samples over 21.82 seconds.

| Measurement | Result |
| --- | --- |
| Mean / peak working set | **18.95 / 18.95 MiB** |
| Mean private memory | **4.16 MiB** |
| CPU across all 12 logical processors | **0.000% at the sampling resolution** |
| Dashboard launch-to-heading observation | **6381 ms**, including test-only CDP setup |

This single fresh-background run excludes the open dashboard, active recording/encoding, local-model inference and shared Windows audio-service memory. The dashboard observation is not a pure cold-start measurement or an improvement claim; it includes test connection overhead and varied machine/cache load. Zero after rounding is not literally no CPU work. See [background data](verification/performance-0.2.1.json).
## Hush 0.3.0 measured run

On the same Intel i5-1235U / 12 logical processors / 7.7 GiB Windows system, a fresh release background run used five seconds of warmup and 20 samples over 22.10 seconds. Mean/peak working set was **18.89/18.89 MiB**, private memory **4.07 MiB**, and CPU rounded to **0.000%** across all logical processors. The resting native pill and stopped microphone preparation were enabled; there was one process and no dashboard or local model. This excludes shared audio-service memory and active processing. [Raw run](verification/performance-0.3.0.json).

The actual dashboard heading appeared after 3440 ms including test-only connection overhead. This is not a controlled cold-start comparison. A cold-first-use microphone worker took 666 ms to Ready, followed by 4/6/3 ms on reuse. A separately prepared worker took 8/6/8 ms to Ready and 73/59/61 ms to first frames. Initial Windows/device preparation is not instantaneous, and these observations are not latency guarantees.

The two synthetic Groq text-polish requests took 567 and 671 ms with `openai/gpt-oss-20b`. That is extra editing time, excluding capture, speech recognition and target insertion. Always adds an edit for every ordinary dictation; Sometimes applies deterministic length/cue/style checks; Never makes no automatic text-model request. Exact-word mode and saved snippet expansions bypass automatic editing. A real end-to-end latency distribution is still unmeasured.

## Hush 0.3.1 responsiveness measurements

Same documented hardware, 2026-10-20. Microphone reads now target a 20 ms interval (50 Hz) during capture, with native update messages coalesced and dashboard metering limited to 10 Hz. Processing animation targets 25 Hz. Opening takes 170 ms, notices 220 ms and closing 130 ms, with a fixed lower edge; reduced motion skips the transition. These configured cadences are not measured keypress-to-display latency.

A real native-window synthetic 50 Hz workload delivered **244 readings in 5 seconds**, consuming **0.1719 CPU seconds / 0.286% across 12 logical processors**, with no new drawing surface after warmup. This was a debug test process, not the optimized whole app, and excludes audio/network/dashboard. Surface/DC/pixel buffers are reused at a stable size, the accessible title is cached and idle has no animation timer. No rendering dependency was added.

Real worker observations after the final gap-recovery change: cold-first-use Ready **249 ms**, subsequent normal-stop sessions **6/4/16 ms**; a separately prepared worker reached Ready **8/3/7 ms** and first frames **38/120/42 ms**. Machine/compiler load varied. Initial preparation is not eliminated and a sub-100 ms hardware-wide promise is unsupported.

The public approximately 11-second JFK speech fixture took **1,342 ms** through actual Groq Turbo with a cold HTTP client, returning 22 words. This excludes speaking time, microphone initialization, local layout, optional AI edits and insertion. It is one request, not a controlled before/after speed comparison or a word-error-rate study. Vocabulary hints now retain complete distinct names within the application request budget; general speech accuracy remains unmeasured.

Packet loss was reproduced on the real endpoint and with a forced reader overrun. The app now preserves the remaining recording for mandatory review rather than ending the entire session; it does not reconstruct missing audio or claim to remove the driver/scheduling cause. Such captures skip automatic insertion, snippets and automatic AI editing. No extra cloud/model request was added to the ordinary fast path.

### Packaged 0.3.1 background run

After compilation and packaged runtime checks ended, a fresh release process with its resting native pill and stopped microphone preparation used **18.99 MiB mean/peak working set** and **4.09 MiB private memory**. CPU rounded to **0.000%** across 12 logical processors during 20 samples over 34.79 seconds, after a five-second warmup. There was one process, no dashboard webview and no local model. Binary SHA-256: `1048f1638557ebfc8344b823aea2b3100275ea226a2780db61e43be65a7eabe7`. [Full sample record](verification/performance-0.3.1.json).

This one background observation is close to the prior 18.89 MiB result, not evidence of a lower idle footprint. Drawing-resource reuse reduces steady animation allocations; no controlled before/after CPU study or end-to-end latency distribution was performed. Recording buffers, encoding, shared audio services, the dashboard and inference add memory outside this measurement. Rounded zero CPU is not literally zero instructions. Actual dashboard heading observation was **3,360 ms**, including test-only CDP overhead, not a controlled cold-start result.

## Hush 0.3.2 compact-state observation

The native bar now contracts after 2.5 seconds to a 96×28 bottom or 28×88 side waveform while capture/processing continues. Native meter traffic advances animation/deadlines as well as timers, so delayed low-priority timer delivery does not keep details open. Idle still has no animation or collapse timer.

A debug synthetic native-window workload, after compact-state warmup, delivered **242 readings in 5 seconds**, consuming **0.1250 CPU seconds / 0.208% across 12 logical processors**, with no new steady-size drawing surface. Microphone capture, network, dashboard and test-only pixel-file capture were excluded. This is a single observation, not a controlled speedup claim or the full application's release CPU. Test-only pixel capture was enabled in a separate visual-inspection run; its disk work is not included in this CPU result. [Component record](verification/update-0.3.2.json).

Microphone timing diagnostics and review thresholds address usability, not general recognition accuracy. A measured 81 ms startup gap no longer blocked three normal recordings; a warmed-stream 500 ms forced loss did require review. Lost speech cannot be reconstructed. No new inference request or resident model was added.

After compilation and packaged checks, a fresh release background run used **18.93 MiB mean/peak working set**, **4.08 MiB private memory** and CPU rounded to **0.000%** across 12 logical processors. Twenty samples ran over 21.93 seconds after five seconds of warmup. There was one process, a resting native pill and stopped microphone preparation, with no dashboard/model resident. The result is close to 0.3.1 and does not establish a reduction in idle memory. Dashboard heading observation was **3,313 ms**, including test-only CDP setup. No new cloud/end-to-end timing was measured. [Raw background data](verification/performance-0.3.2.json).

## Hush 0.3.3 bounded processing and recovery

Same documented hardware, fresh release process after compiler/packaged checks finished: **19.21 MiB mean/peak working set**, **4.17 MiB private memory**, **0.023% CPU across 12 logical processors**. Five seconds of warmup preceded 20 samples over 23.05 seconds. One background process had a resting native bar and stopped microphone preparation, with no dashboard/model or saved private audio resident. This is a single observation, not a controlled speed or footprint improvement. Retained private audio consumes additional memory, bounded to three clips; active encoding and inference remain outside this measurement. [Raw run](verification/performance-0.3.3.json).

The actual dashboard heading was observed after **7,481 ms**, including test-only CDP initialization and variable local system/cache load. This is not a pure cold-start measurement or evidence of faster startup. No new live Groq/end-to-end latency was measured. A five-second configurable speech deadline now bounds waiting before error/recovery, rather than making inference faster. Optional cleanup separately falls back to local text after five seconds; selected-text editing has a ten-second limit. No resident model, idle expiry poll or extra ordinary cloud request was added.

## Hush 0.4.0 recording and editing measurements

Same documented Windows hardware. First actual microphone samples now determine Ready rather than successful device initialization alone. Three prepared-worker sessions reached Ready in **46/43/122 ms**, with first meter frames in **46/44/122 ms**, then cancelled cleanly without files or upload. These component times exclude the default **300 ms preparation pause**, shortcut recognition and cue playback. The intentional pause is adjustable from 0 to 1,500 ms. System load and hardware vary; these are not end-to-end guarantees or a controlled startup speed comparison.

Public synthetic Groq requests using the real clients and current style pipeline took **1,279 ms** for Sometimes and **669 ms** for Always routine editing with `openai/gpt-oss-20b`. Never made no automatic editing request. The configured Command Mode model, `openai/gpt-oss-120b`, took **842 ms** for Clarify and **734 ms** for Expand. The tested facts and URL survived; the Expand additions were explicitly labeled. These single observations can include cold model discovery and exclude microphone capture, transcription and actual target insertion. They do not establish general accuracy or a latency distribution. [Sanitized component data](verification/update-0.4.0.json).

Preparing/processing use a dark static waveform; only recording responds to meter levels. Diagnostics retain at most 200 stage/outcome/timing events across 50 generations in memory, without text/audio/keys. Startup is bounded to five seconds after the preparation pause, delivery to two seconds, prompt editing to twenty seconds; ordinary speech and five-second cleanup fallback preferences remain independent. Deadline handling bounds waiting and preserves results; it does not accelerate provider inference. Windows cannot revoke a synchronous accessibility mutation already accepted by an external provider; cancellation is checked immediately before dispatch. A live microphone-to-Notepad/browser latency measurement remains unverified because the native test environment could not acquire the foreground field.

After compiler and packaged checks ended, a fresh release background run used **21.59 MiB mean/peak working set**, **4.64 MiB private memory** and **0.012% CPU across 12 logical processors**. There was one background process with a resting native bar and stopped microphone preparation, without dashboard or local model. The five-second warmup and twenty samples are recorded in the [raw report](verification/performance-0.4.0.json). This is above the prior 19.21 MiB observation and does not establish a memory improvement; CPU and startup observations also lack a controlled before/after study. Dashboard heading was observed after 6,115 ms including test-only CDP overhead. Active audio, encoding, recovery clips and model inference require additional memory. No lower end-to-end speech latency or general accuracy claim is made.

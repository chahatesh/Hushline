# Performance report

No end-to-end transcription latency, under-100 MB idle footprint, or sub-100 ms recording-feedback claim is made without a measurement.

## Environment and methodology

Local implementation/testing on Windows NT 10.0.26300.0, x64, Intel Core i5-1235U, 12 logical processors, 7.7 GiB reported physical memory. The Rust compiler reports stable 1.99.1/MSVC; Node 22.14.0. Microsoft compiler/SDK files were extracted into the ignored workspace-local tools directory. The build was running during early smoke tests; those samples are not a quiet-machine benchmark. CPU/RAM and version information are recorded by `scripts/benchmark.ps1` when an optimized executable is available.

## Recorded results

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
- Audio capture buffers are bounded/preallocated; samples are downmixed in the callback. Energy metering is limited to 20 Hz while recording.
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

Profile the observed slow microphone initialization before changing ownership/lifetime of WASAPI resources. Investigate safe descriptor/configuration warmup and stable device IDs without silently keeping a microphone stream open. Verify dashboard release/reopen memory behavior and mixed-DPI overlay latency. The under-100 MB target remains a target until the entire process tree is measured.

## Final native-overlay build measurements

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

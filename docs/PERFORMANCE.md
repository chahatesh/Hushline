# Performance

Hush keeps recording, shortcuts, networking, and insertion in Rust. The dashboard webview is created on demand and destroyed when closed. The voice bar is native, with no idle animation loop. Audio uses bounded buffers and 16 kHz mono WAV; the HTTP client reuses connections. Plain dictation avoids an extra language-model call.

## Recorded observations

Hardware: Intel Core i5-1235U, 12 logical processors, 7.7 GiB RAM, Windows 10.0.26300 x64. These are historical single-machine observations, not guarantees for the current build.

| Observation | Result | Limits |
| --- | --- | --- |
| 0.3.3 release background working set | 19.21 MiB | One process; no dashboard, local model or retained private recordings. |
| Same run private memory / CPU | 4.17 MiB / 0.023% | CPU across all 12 logical processors; 20 samples after five-second warmup. |
| 0.4.0 prepared microphone first samples | 46 / 44 / 122 ms | Three cancelled sessions; excludes the configurable 300 ms startup pause. |
| 0.6.0 Groq Turbo public speech fixture | 1,152 ms | One approximately 11-second JFK WAV, 22 words, cold HTTP client. Excludes capture, editing and insertion. |
| Whisper Tiny public fixture on CPU | 2,751 ms | One file recognition; excludes capture and insertion. |
| Optional qwen3:0.6b local text model | 523 MB download; about 571 MiB observed process-tree working set | Small download does not imply negligible active memory. |
| Same model first / subsequent short edit | 14,616 / 3,403 ms | Loaded machine, four CPU threads, 2,048-token context. One longer edit took 10,800 ms and did not add paragraphs. |

No full end-to-end latency distribution, general accuracy score, current cold-start percentile, or negligible-memory local-LLM claim has been established. Active recording, retained private audio, dashboard WebView2, and inference increase memory.

## Measuring a build

Build an optimized executable, quit any existing instance, and use a quiet machine:

```powershell
powershell -NoProfile -File scripts/benchmark.ps1
```

The script launches an owned release process, warms up for five seconds, samples working set/private memory/CPU, writes `artifacts/performance.json`, then stops that process. Repeat runs and report the version, hardware, active windows/models, sample period, and all measured stages. Summed working sets can double-count shared pages.

`scripts/benchmark.ps1` measures background resources. It does not measure microphone-to-paste latency. The optional `tests/native/local-model-benchmark.mjs` downloads an isolated model and uses synthetic text; it does not configure a daily-use model automatically.

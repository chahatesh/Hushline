# Small local text model trial

A small LLM can run locally, but it cannot provide instant loading and negligible active memory simultaneously. Ordinary Hushline cleanup, dictionary processing, paragraphs and clear lists therefore remain model-free by default. Offline speech and optional AI editing are separate choices.

The opt-in benchmark downloaded **qwen3:0.6b, Q4_K_M**, using the already installed Ollama 0.12.5 executable. It started its own loopback-only service on port 11439 with cloud disabled, a workspace-only model directory, four CPU threads and a 2,048-token context. Only three synthetic editing examples were sent. The benchmark service and its children were stopped afterward. No microphone, saved Groq key, private transcript, existing Ollama server or system startup setting was used or changed.

| Observation | Result |
| --- | --- |
| Model file | 522,653,767 bytes; tag digest `7df6b6e09427a769808717c0a93cadc4ae99ed4eb8bf5ca557c90846becea435` |
| First short edit / model load component | 14,616 / 10,015 ms |
| Subsequent short list edit | 3,403 ms |
| Subsequent longer paragraph edit | 10,800 ms |
| Server-only working set before inference | 42.82 MiB |
| Observed service/process-tree working set after first edit | 570.95 MiB |

Hardware: Intel Core i5-1235U, 12 logical processors, 7.7 GiB RAM, Windows NT 10.0.26300. A Rust build was running concurrently, so these are loaded-machine observations, not a quiet-machine speed ranking. Working set varied substantially under memory pressure; later smaller working sets are not evidence that the model needs almost no RAM. Disk size is not a RAM ceiling. Other hardware, context sizes and generation lengths change both memory and time.

The first example corrected stutters and preserved its date/time/version. The list example produced Markdown hyphens instead of the requested • marker. The longer example retained its wording but **did not add paragraphs**. Three examples cannot establish general accuracy. These results do not justify an automatic extra model call for every dictation.

For optional local rewriting, install/use a local Ollama model under the normal Offline settings. The production app uses port 11434, rejects remote model metadata, disables thinking, requests enough context for the input/output, and unloads the model after each edit. The isolated benchmark download stays in `.tools/local-text/models` and is not bundled in the installer or automatically selected for daily dictation. Keeping a model resident can reduce later load time but retains memory; Hushline currently favors releasing it.

Reproduce explicitly with an installed Ollama executable:

```powershell
node tests/native/local-model-benchmark.mjs "C:\path\to\ollama.exe"
```

The script downloads approximately 523 MB into the ignored workspace folder and writes its report to `artifacts/local-model-benchmark.json`. It sends synthetic text only. Model/runtime licenses remain theirs: [model and download details](https://ollama.com/library/qwen3:0.6b), [official Qwen model](https://huggingface.co/Qwen/Qwen3-0.6B), [Ollama local-only configuration](https://docs.ollama.com/faq#how-do-i-disable-ollamas-cloud-features).

Related implementation references: [Microsoft Start/Stop capture](https://learn.microsoft.com/en-us/windows/win32/coreaudio/capturing-a-stream), [Groq reasoning controls](https://console.groq.com/docs/reasoning).

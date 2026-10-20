# Offline dictation

Hush 0.2 can transcribe speech, format notes and paragraphs, apply dictionary rules and snippets, and retain local history without an internet connection after setup. No Groq API key is needed in offline mode. It never silently uploads a recording or falls back to cloud inference.

## Speech setup

1. Open Settings → Offline. Choose Tiny (77,691,713 bytes) for lighter CPU work, or Base (147,951,465 bytes) for a larger model. Both are multilingual.
2. Choose Download speech pack. The first install also downloads an 8,194,445-byte Windows x64 CPU engine. This is the only online stage for speech setup.
3. Choose Use offline dictation, or select On this computer in General. Save settings.
4. Select and test your microphone. Focus an editable field and use the same dictation shortcut as cloud mode.

The app pins whisper.cpp v1.9.2 and a Hugging Face model repository revision, verifies exact size and SHA-256 before installation, and stages the engine before making it available. Cancelled or incomplete downloads are removed. A model with an unknown name cannot introduce a URL or path. Upstream licenses accompany the engine.

Models live under `%LOCALAPPDATA%\io.chahatesh.hushline\offline`. Copying this complete directory to the same location on another Windows installation avoids another download. Configuration backups do not include model binaries. Offline speech needs the Microsoft Visual C++ runtime required by the upstream Windows engine. If it fails to launch, install Microsoft's x64 Visual C++ redistributable or rebuild the engine from upstream.

## Formatting and commands

Automatic layout, fillers, dictionary, and snippets are local and require no text model. English transformations run only for identified English; multilingual speech recognition remains available. Literal mode preserves wording and layout.

Advanced rewriting and Command Mode require an optional downloaded local text model. Install [Ollama for Windows](https://ollama.com/download/windows), download a model suitable for your hardware (for example `ollama pull llama3.2:3b`), and configure Ollama with `OLLAMA_NO_CLOUD=1`. Restart Ollama after changing that setting. Ollama also documents `disable_ollama_cloud` in its configuration file. See [its official local-only instructions](https://docs.ollama.com/faq#how-do-i-disable-ollamas-cloud-features).

Enter the local model's name under Offline → Optional offline AI editing. Hush connects exclusively to `http://127.0.0.1:11434`, disables proxies and redirects for that client, rejects cloud-tagged/remote model metadata, and checks the model before sending selected text. It never invokes web search or tools. The model is requested to unload after each edit. This lowers retained memory at the cost of reloading for the next edit. Hush does not install or start the Ollama service automatically.

If local editing is unavailable, ordinary dictation with local formatting still works. Enhanced cleanup falls back to local cleanup with a visible warning. Command Mode keeps a failed edit out of the target application. Hush's trust boundary includes the local service you choose to run; configure it for local-only operation as described above.

## Privacy and performance

Raw WAV is piped to the speech engine through standard input. It is never saved as an audio file. A temporary JSON transcript is written inside an automatically cleaned temporary directory, then removed on completion or cancellation. A forced OS/process crash can leave a temporary transcript directory; remove `transcript-*` directories from the offline folder when Hush is closed if needed. Local history is controlled separately by the private-mode setting. The local database is not encrypted.

The speech model is loaded by a hidden child process only while transcribing; it is not resident while Hush rests. Cancellation kills and reaps that process. CPU-only inference can be slower than Groq, particularly for long recordings and larger models. The model-size labels describe downloads, not maximum RAM use. See [measured results](PERFORMANCE.md); there is no blanket speed or accuracy guarantee.

Upstream: [whisper.cpp](https://github.com/ggml-org/whisper.cpp/tree/v1.9.2), [model repository](https://huggingface.co/ggerganov/whisper.cpp), [Whisper](https://github.com/openai/whisper). Those projects, weights, and trademarks retain their own ownership and licenses.

## Small models and long edits

A real qwen3:0.6b trial is documented in [LOCAL_MODEL.md](LOCAL_MODEL.md). A small download still needs substantial active memory and can take seconds to load/edit; it is not an instant, negligible-RAM replacement for local formatting. The benchmark model was isolated from the normal Ollama profile and is not bundled or selected automatically.

The editing client disables optional thinking, reserves explicit input/output context, and refuses text that exceeds a safe local budget instead of allowing silent prompt truncation. Automatic cleanup also checks for missing identifiers, changed numeric content and severe shortening before accepting a rewrite. Those checks are conservative and do not prove semantic accuracy. Every Command Mode edit still needs its preview/selection safeguards.

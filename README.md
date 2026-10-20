<p align="center"><img src="public/logo.svg" width="76" alt="Hushline original waveform logo"></p>
<h1 align="center">Hushline</h1>
<p align="center">A little less typing. A lot more you.</p>
<p align="center"><a href="https://github.com/chahatesh/Hushline/actions/workflows/ci.yml"><img alt="Windows build" src="https://github.com/chahatesh/Hushline/actions/workflows/ci.yml/badge.svg"></a><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-2b4f8d"><img alt="Platform Windows 10 and 11" src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-64748b"><img alt="Status pre-release" src="https://img.shields.io/badge/status-pre--release-c08436"></p>

Hushline is an independent, open-source Windows dictation application built with Rust, Tauri 2, and SolidJS. Hold a shortcut, say what you mean, and insert the result in the field where you were working. Choose Groq cloud recognition with your own API key, or download a local Whisper pack for offline dictation. Local automatic layout adds notes, bullets, and paragraphs without another AI call.

**Release status: pre-release, with live acceptance pending.** The native pipeline and desktop interface are implemented, but live speech → Groq → insertion has not been verified in this environment. Do not treat this as a production-complete Wispr Flow replacement. The 0.2.1 build averaged 18.95 MiB working set and 4.16 MiB private memory in one documented 20-sample background run with stopped microphone preparation enabled; an open dashboard or active local model adds resource use. Silent installation, native settings/key storage, microphone stopping/cancellation, and dashboard close/reopen passed. A public Groq speech fixture also passed. The exact feature and test status is recorded in [FEATURES](docs/FEATURES.md), [QA](docs/QA.md), and [performance report](docs/PERFORMANCE.md). A public release is blocked until core acceptance checks pass.

Hushline is not affiliated with, endorsed by, or an official product of Wispr Flow. All project branding and original code were created independently.

![Hushline dashboard, empty browser interface preview](docs/screenshots/home.png)

## What is here

- Push-to-talk, hands-free recording, editable shortcuts, optional double-tap activation, Escape cancellation, tray lifecycle, and a nonactivating waveform overlay.
- A smaller, smooth-edged voice pill without a mic icon. Fast startup prepares a stopped microphone client; the recorder reads audio only after activation. Default recordings allow ten minutes, with silence stopping optional.
- Native WASAPI capture through CPAL, energy-based silence detection, bounded recording duration, in-memory 16 kHz mono WAV encoding, and asynchronous Groq speech requests.
- Optional offline Whisper Tiny/Base packs with verified downloads, local notes/bullets/paragraph layout, and optional editing through an installed local Ollama model. Tiny file transcription was tested; live offline delivery and Ollama editing remain unverified.
- Fast mode by default: Whisper transcription → conservative local cleanup → insertion. An extra text-model call happens only when enhanced/polished processing or a matching AI writing style is enabled, or in Command Mode.
- Windows protection/focus checks, default one-action clipboard paste with guarded restoration, optional compatibility Unicode typing, and last-transcript recovery. Changed/unknown targets retain text for manual paste.
- SQLite history, personal dictionary, exact-trigger multiline snippets with previews, writing styles by executable name, scratchpad, statistics, and configuration backup.
- Windows Credential Manager for the key, optional local history, no Hushline account, no telemetry, no subscription, and no artificial application usage cap. **Groq's real charges and rate limits still apply.**

These are implemented code paths, not a blanket compatibility claim. Terminal automatic insertion, browser-domain styles, rich-text snippets, a signed updater, and several advanced formatting capabilities are not supported. See the feature matrix before relying on a specific workflow.

## Install

Source is available at [chahatesh/Hushline](https://github.com/chahatesh/Hushline). No public GitHub Release is available yet; acceptance verification remains required. Locally built artifacts, when generated, are in:

- `src-tauri/target/release/bundle/nsis/Hushline_0.2.1_x64-setup.exe` — per-user installer with shortcuts and uninstaller.
- `artifacts/Hushline_0.2.1_x64_portable.zip` — executable plus license/readme. This uses the same Windows app-data settings; it is portable as a binary, not a fully isolated profile.

The installer is unsigned unless a maintainer signs it with their own certificate. It requires no administrator permissions for ordinary per-user use. WebView2 installation may be required. Ordinary updates retain app-data settings. Local history and the Groq key may remain after uninstall so an update/reinstall does not unexpectedly erase them; remove the key and clear history in the app before uninstalling if you want them deleted. Remove the remaining app-data directory yourself for complete removal. Startup is opt-in under Settings; no service is installed.

### Requirements

- Windows 10 or Windows 11, x64; interactive desktop session.
- Microsoft Edge WebView2 Runtime (the installer can download Microsoft's bootstrapper).
- A working microphone and Windows microphone privacy permission for desktop apps.
- For cloud mode: internet access to Groq and your own API key. For offline mode: a one-time 78 MB Tiny or 148 MB Base model download plus an 8 MB engine. No key or internet is needed for local transcription after installation. See [offline setup](docs/OFFLINE.md).
- The target application must run at an integrity level Windows permits Hushline to interact with. Hushline does not request elevation or bypass protected input boundaries.

## First dictation

1. Launch Hushline and open **Settings → General**.
2. Create a key at [Groq Console](https://console.groq.com/keys), enter it in the password field, and choose **Save key**. Verify the key and refresh available models if desired.
3. Choose a microphone and save settings. Use **Test** to check capture; microphone tests send no audio to Groq.
4. Focus an editable field in Notepad. Hold **Ctrl + Win**, speak, then release.
5. Stay in that field until processing ends. If focus changes, the text is preserved in Hushline; copy it or focus your intended field and use the paste-last shortcut.

Fast microphone startup is on by default. After initial preparation, three native trials delivered audio frames in 21–68 ms here; this is not a hardware-wide guarantee. Preparation itself is asynchronous and can take around a second. Disable it in General if you prefer releasing the stopped audio client between recordings. Neither mode reads idle audio. Cloud recognition starts after you finish speaking; a public 11-second fixture took 2.54 seconds in one real Groq request. Insertion/end-to-end latency remains separately unverified.

For longer speech, automatic layout adds paragraphs and turns clear lists into • bullets without an LLM. Extra AI polishing is optional. A small local-model trial added 3–15 seconds and substantial active memory; see [the measured tradeoff](docs/LOCAL_MODEL.md). It is not enabled automatically.

The default speech model is `whisper-large-v3-turbo`. Choose `whisper-large-v3` for the accuracy option. Automatic language detection is the default; a selected language sends the supported `language` parameter. Verbose transcription metadata identifies the language without an extra inference call. English filler/formatting rules run only for confirmed English; other or unknown languages preserve recognized wording locally, with optional explicit AI processing. Dictionary hints use `prompt`; the app does not invent a streaming endpoint. Available text models are checked before AI editing, with `openai/gpt-oss-20b` and `openai/gpt-oss-120b` as configurable defaults. See [Groq speech documentation](https://console.groq.com/docs/speech-to-text), [API reference](https://console.groq.com/docs/api-reference), and [model list](https://console.groq.com/docs/models).

### Keyboard shortcuts

| Action | Default |
| --- | --- |
| Push-to-talk, hold/release | Ctrl + Win |
| Hands-free, toggle | Ctrl + Win + Space |
| Command Mode, hold/release | Ctrl + Win + Alt |
| Paste last transcript into current field | Ctrl + Win + V |
| Copy last transcript | Ctrl + Win + C |
| Open dashboard | Ctrl + Win + H |
| Open scratchpad | Ctrl + Win + J |
| Cancel active dictation/edit | Escape |

Shortcuts can use modifiers, one letter/number/F-key/Space, or Mouse4/Mouse5. Duplicate assignments within Hushline are rejected. Other applications and Windows may intercept a shared shortcut; Hushline cannot universally detect another process's low-level hook. Modifier-only chords need at least two modifiers.


### Offline, formatting, and the voice bar

Choose **Settings → Offline**, select Tiny or Base, and download the verified pack. Select **On this computer** under General and save. Offline mode does not require a Groq key and never falls back to a cloud provider. For optional advanced local rewriting, see [Ollama setup](docs/OFFLINE.md).

**Settings → Dictation → Automatic layout** can detect explicit notes and ordered lists, force notes or paragraphs, or leave layout unchanged. For example, “Meeting notes. First, review the draft. Second, send it to Maya.” becomes a heading and two bullets. Literal mode preserves layout. Automatic paragraph breaks use sentence boundaries; ambiguous speech and non-English conventions may require explicit “new paragraph” or optional AI editing.

The **Voice bar** tab docks the native pill at the bottom, left, or right. It stays translucent and still at rest, expands only slightly, and becomes opaque during capture. Click it for hands-free mode, click × to cancel, or right-click to open the dashboard. It does not take keyboard focus. No animations run at rest.

Shortcut releases no longer fall back into a shorter modifier-only chord. Hold-to-talk is stopped by release or the maximum duration, not the silence timeout. For Windows shortcut conflicts, choose **Shortcuts → Use dedicated-key shortcuts** or record your own combination with the keyboard button.

### Command Mode

Enable **Allow Command Mode to read selected text** in Settings first. Select text in the target app, hold the Command Mode shortcut, and say an editing instruction. Hushline transcribes the instruction, sends only the selected text and instruction to Groq (or your configured local Ollama model in offline mode), and shows an edit preview. Approval may bring the original app forward, but replacement proceeds only if the field and selection still match. If access is unsupported or a selection changes, copy the edit instead. Hushline never executes terminal commands, sends messages, or submits forms as a voice action.

### Dictionary, snippets, and styles

Add a dictionary word with a blank replacement for a vocabulary hint, or enter an exact whole-phrase replacement. Rules apply in one pass, without chained substitutions. Dictionary import/export uses JSON arrays of `{ "source": "groak", "replacement": "Groq" }` objects.

A snippet expands only when its trigger is the entire utterance. It supports multiline plain text, `{{date}}`, `{{time}}`, and optional confirmation. Rich text is not preserved. App styles match executable names such as `Code.exe` or `WINWORD.EXE` when context matching is enabled. No screenshot, browser-domain inspection, or surrounding-text collection is performed.

## Privacy

The key is stored in Windows Credential Manager, not in SQLite, frontend assets, environment files, or logs. Your current Windows account is the trust boundary. Raw audio stays in memory and is released after processing/cancellation; it is never saved to an audio file. Audio already uploaded cannot be recalled by cancelling.

In cloud mode, audio is sent to Groq for dictation. In offline mode, it is piped to a local whisper.cpp process, with no cloud fallback. The local engine writes a temporary JSON transcript (no raw audio); it is deleted after processing. Offline models live under `%LOCALAPPDATA%\io.chahatesh.hushline\offline`. Text goes to Groq only for enabled AI processing or Command Mode; vocabulary hints also accompany speech requests. Groq's own data policies apply. Hushline has no telemetry or cloud history store. Local data lives in `%APPDATA%\io.chahatesh.hushline\hushline.sqlite` (the Tauri user app-data directory); it is not database-encrypted. Disable local history for private mode and clear existing records separately. Configuration backups exclude API keys, history, and scratchpad content.

Clipboard mode snapshots supported global-memory formats, refuses unsafe/oversized snapshots, and restores only if the clipboard sequence still matches. It leaves newer copied content alone. Some apps read clipboard contents asynchronously, so universal paste verification is not possible. Compatibility Unicode typing is opt-in and avoids clipboard changes; paste is the default. It preserves multiline output for copying instead of typing Enter, which could submit a message; choose clipboard insertion for multiline paste. Delivery is reported conservatively; check your text before sending it.

## Develop and build

Install [Tauri's Windows prerequisites](https://v2.tauri.app/start/prerequisites/): Node.js 22 LTS, Rust 1.90 or newer with the MSVC host (tested with 1.99.1), Microsoft C++ Build Tools and Windows SDK, and WebView2. Use a Visual Studio Developer PowerShell if the compiler is not discovered automatically. If PowerShell blocks `npm.ps1`, use `npm.cmd`; changing machine execution policy is unnecessary.

```powershell
npm ci
npm run desktop
```

Build and package:

```powershell
npm run build
npm run package
npm run portable
```

Run checks:

```powershell
npm run lint
npm test
npm run build
npx playwright install chromium
npm run test:ui
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

The ignored native test opens owned Windows edit controls and temporarily tests clipboard/input safety. Run it only on an interactive Windows desktop while you are not typing/copying:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml native_input_clipboard_focus_and_password -- --ignored --test-threads=1 --nocapture
```

`npm run dev` alone opens a clearly labeled browser UI preview. It cannot record, store API keys, call Groq, or insert into Windows apps, and is not an end-to-end demo. Preview collection data lives separately in browser storage. The Playwright tests exercise interface behavior against compiled assets; they do not certify dictation.

No Groq key is needed to compile or run automated unit tests. Loopback HTTP fixtures test errors and documented request structure. See [architecture](ARCHITECTURE.md), [contribution guidance](CONTRIBUTING.md), and [publishing instructions](docs/PUBLISHING.md). Build/release badges are deliberately not linked to a nonexistent repository or release; add the actual URLs after publishing.

## Troubleshooting

| Symptom | What to check |
| --- | --- |
| No microphone or capture failure | Reconnect the device, refresh the list, check Windows microphone permissions, and save another input selection. |
| No speech detected | Test the microphone, speak for longer than a quick tap, or lower the activity threshold slightly. This is energy detection; noise can also count as activity. |
| Key rejected | Replace the key in Settings and verify it with Groq model discovery. |
| Rate limit | Wait before retrying; check your Groq account's actual limits. Hushline caps automatic retries and honors long retry delays by returning an error. |
| Text saved instead of inserted | Focus, caret, selection, or accessibility identity changed; the field is protected/read-only; or Windows blocked input. Recover the transcript manually. |
| Terminal dictation not inserted | Automatic terminal insertion is deliberately disabled. Copy and paste manually after reviewing the text. Integrated terminal behavior still needs compatibility testing. |
| Clipboard cannot be restored | Use the default Unicode input method, or copy manually. Hushline refuses unsupported formats before changing them. |
| Command Mode cannot read a selection | Enable selected-text permission and use an app that exposes a supported UI Automation selection. |
| Dashboard closed but app remains running | This is the tray behavior. Use the tray menu's Quit action to exit. Reopening recreates the dashboard. Save scratchpad/settings drafts before closing. |
| A Windows shortcut is intercepted | Choose a different binding. Low-level shortcuts used by other software cannot all be enumerated. |

## Author and license

**Created by Chahatesh**

**License: MIT**

Copyright (c) 2026 Chahatesh. The original project code and assets are licensed under the complete [MIT License](LICENSE). Dependency code, Microsoft components, trademarks, and third-party assets retain their own licenses and ownership. See [SECURITY](SECURITY.md) for reporting and trust boundaries. Bundled dependency notices and full distributed license texts are in [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md) and [THIRD_PARTY_LICENSES](THIRD_PARTY_LICENSES.txt).


<p align="center"><img src="public/logo.svg" width="76" alt="Hush waveform logo"></p>
<h1 align="center">Hush</h1>
<p align="center">A little less typing. A lot more you.</p>
<p align="center"><a href="https://github.com/chahatesh/Hushline/actions/workflows/ci.yml"><img alt="Windows build" src="https://github.com/chahatesh/Hushline/actions/workflows/ci.yml/badge.svg"></a> <img alt="MIT license" src="https://img.shields.io/badge/license-MIT-2b4f8d"> <img alt="Windows 10 and 11" src="https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-64748b"></p>

Hush is an open-source Windows dictation app. Hold **Ctrl + Win**, speak, then release to paste into the field where you were working. Use Groq with your own API key or download a Whisper speech pack for offline recognition.

No Hush account, subscription, telemetry, or application usage limits. Groq's charges and rate limits apply to cloud inference.

**Pre-release:** Windows app compatibility and live dictation testing are still in progress. Current builds are unsigned; see the [signing policy](CODE_SIGNING_POLICY.md). There is no public GitHub Release yet.

![Hush dashboard](docs/screenshots/home.png)

![Hush voice bar](docs/screenshots/native-pill.png)

## Features

- Hold-to-talk and hands-free dictation with customizable keyboard and mouse shortcuts.
- A native, nonactivating voice bar with bottom and side docking, a recording timer, and sound-reactive feedback.
- Casual, Standard, and Formal writing styles, with local paragraphs, lists, dictionary rules, and spoken punctuation.
- Independent Always, Sometimes, and Never AI-polishing choices. Plain dictation needs only one speech request.
- Personal dictionary, exact-trigger multiline snippets, app-specific styles, and a scratchpad.
- Local transcript and recording history. Listen, download a WAV, or retry recognition without recording again.
- Command Mode for selected text and Improve prompt for a whole editable field. Edits require preview approval.
- Secure API-key storage in Windows Credential Manager and encrypted saved recordings.

See [feature status](docs/FEATURES.md) for supported behavior and compatibility limits.

## Installation

Requires Windows 10/11 x64, Microsoft Edge WebView2, a microphone, and desktop microphone permission. Normal use requires no administrator access or background service.

Build from source using the instructions below, or obtain a development installer from a successful [Windows build](https://github.com/chahatesh/Hushline/actions/workflows/ci.yml). Development artifacts are labeled unverified. A build produces a per-user installer and a portable executable ZIP; the portable build shares the usual Windows app-data profile.

The installer creates **Hush Dictation** shortcuts. Settings are retained through ordinary updates. **Settings → General → Start with Windows** is optional. For a complete uninstall, remove the saved key, transcript history, and recordings in Hush first; profile data is retained by default.

## Getting started

1. Open **Settings → General** and save your [Groq API key](https://console.groq.com/keys). For local recognition instead, follow [offline setup](docs/OFFLINE.md).
2. Select your microphone and choose **Test**. Microphone testing does not upload audio.
3. Focus an editable field, for example in Notepad. Hold **Ctrl + Win**, wait for the ready sound, speak, then release.
4. Keep that window active while Hush processes your words. Text is inserted in one operation where the app supports it, with clipboard paste as the fallback.

The voice bar stays dark during preparation and processing. Its waveform reacts only after microphone samples arrive. The startup pause defaults to 300 ms and is adjustable. Release during preparation cancels that hold.

The default cloud speech model is `whisper-large-v3-turbo`; select `whisper-large-v3` for the accuracy option. Language detection is automatic unless you select a language. Cloud recognition starts when recording ends; Groq does not provide a Whisper streaming endpoint here.

## Shortcuts

| Action | Default |
| --- | --- |
| Hold to dictate | Ctrl + Win |
| Toggle hands-free | Ctrl + Win + Space |
| Hold for Command Mode | Ctrl + Win + Alt |
| Improve the focused prompt | Ctrl + Shift + P |
| Paste last transcript | Ctrl + Win + V |
| Copy last transcript | Ctrl + Win + C |
| Open Hush | Ctrl + Win + H |
| Open scratchpad | Ctrl + Win + J |
| Cancel | Escape |

Edit these under **Settings → Shortcuts**. Optional double-tap activation preserves your preference. During hands-free recording, press and release Ctrl + Win again to finish. Hush refreshes its shortcut hooks after desktop changes, wake, or reopening the dashboard. If another app intercepts a chord, use the dedicated-key preset or choose another combination.

Right-click the voice bar for **Paste last transcript**, **Copy last transcript**, **Improve prompt**, **Transcription history**, **Settings**, and **Quit Hush**. The tray menu provides copy, history, settings, and widget controls. **Hide widget for 1 hour** hides the resting bar temporarily; shortcuts and recording feedback still work. Restore it early with **Show widget** in the tray menu. Hiding it does not change your docking preference. With history enabled, Paste last transcript also recovers the latest saved result after restarting Hush; private mode starts without that recovery.

## Writing and editing

| Style | Behavior |
| --- | --- |
| Casual | Normal capitalization without added periods or automatic paragraph/list layout. |
| Standard | Sentence punctuation, readable paragraphs, and clearly detected lists. |
| Formal | Standard layout plus professional grammar and concise wording when AI polish is enabled. |

Choose **Always / Sometimes / Never** AI polish independently of style. Never keeps automatic text cleanup local. Exact-word mode preserves recognized wording and bypasses cleanup. Explicit spoken commands such as “new paragraph” and “bullet point” work in every style except exact-word mode. English cleanup runs only for identified English; recognition supports other languages too.

A clear correction such as “I have three dogs. Wait, no, I actually have four” can become “I have four dogs.” Ambiguous corrections are preserved. Automatic AI cleanup falls back to local text if it times out or fails validation.

Command Mode reads only an explicitly selected range after you enable permission. To improve an existing prompt, focus its text field and press **Ctrl + Shift + P**. Hush reads that field, offers Clarify or Expand, and brings the editable proposal into view. Choose **Apply to original field** to replace it, or Copy/Cancel. Both show an editable preview before replacement. Changed, protected, terminal, or unsupported fields are refused; Hush never executes dictated commands or submits messages.

## Recordings and privacy

Recordings are retained before recognition so a failed request can be retried from **History**. Saved audio uses Windows DPAPI encryption for your Windows account. Retention is capped at 30 days, 1,000 recordings, and 512 MiB. Private mode or disabled recording storage keeps only the latest three recordings in memory, capped at 64 MiB and lost on exit. Exported WAV files are ordinary unencrypted audio.

Cloud mode sends necessary audio and optional editing text to Groq. Offline mode has no cloud fallback. Transcript history and settings stay on your computer and are not encrypted. Password fields and terminal automatic insertion are unsupported. If focus changes or Windows prevents safe delivery, use **Paste here** on the recovery card or the paste-last shortcut in your intended field.

Read the [privacy policy](PRIVACY.md), [offline guide](docs/OFFLINE.md), and [security policy](SECURITY.md).

## Build from source

Install Node.js 22, stable Rust with the MSVC toolchain, Microsoft C++ Build Tools with the Windows SDK, and WebView2.

```powershell
npm ci
npm run desktop
```

Build the Windows installer and portable ZIP:

```powershell
npm run package
npm run portable
```

Output is under `src-tauri/target/release/bundle/nsis/` and `artifacts/`. No API key is needed to build or run automated tests. `npm run dev` runs the browser interface only; native dictation requires the desktop app.

See [CONTRIBUTING](CONTRIBUTING.md), [architecture](ARCHITECTURE.md), [testing](docs/QA.md), and [performance measurements](docs/PERFORMANCE.md).

## Troubleshooting

| Problem | What to do |
| --- | --- |
| Ctrl + Win does nothing | Reopen Hush to refresh shortcuts. Check the saved binding and other apps using the same chord. |
| Microphone does not start | Check desktop microphone permission, the selected device, and exclusive access from other apps. |
| Recognition fails | Check the key/network or offline pack, then retry the saved recording in History. Requests have bounded retries and waiting times. |
| Groq rate limit | Wait before retrying and check your Groq account limits. |
| Text is retained instead of pasted | Refocus your intended editable field and use Paste here or Ctrl + Win + V. Windows protection/focus restrictions still apply. |
| App remains after closing the dashboard | Hush stays in the tray. Choose Quit to exit. |

Hush was formerly Hushline. Internal application identifiers, storage locations, and the repository URL are retained so updates preserve your profile.

Hush is an independent project, not affiliated with, endorsed by, or an official product of Wispr Flow.

## Author and license

**Created by Chahatesh**

**License: MIT**

Copyright (c) 2026 Chahatesh. Original code and assets use the [MIT License](LICENSE). Dependencies retain their licenses; see [third-party notices](THIRD_PARTY_NOTICES.md) and [license texts](THIRD_PARTY_LICENSES.txt).

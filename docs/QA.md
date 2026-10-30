# Testing Hush

Automated checks cover the frontend, Rust components, local HTTP failures, audio retention, and packaging. They do not establish universal Windows application compatibility or replace live microphone-to-field testing.

## Automated checks

```powershell
npm ci
npm run lint
npm test
npm run build
npx playwright install chromium
npm run test:ui
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

The ordinary test suite uses dummy keys and synthetic local HTTP/audio fixtures. It does not upload microphone recordings or use a saved Groq key. Browser tests run the interface preview, with no native microphone or text injection.

Windows CI also tests a real offline CPU transcription fixture, builds the installer and portable ZIP, and exercises the external-signing packaging path. Signing tests use a temporary untrusted certificate without installing a trusted root. Their files are excluded from distribution artifacts.

Regression fixtures cover text history beyond 1,000 records and older years, migration/restart, retries without double counting, cache/deletion behavior, exact 10,000-word boundaries, leap years, local dates, streaks, chart ranges, pagination/search/export, transient prompt drafts, editing preferences and explicit preview approval. Keyword-derived profiles are deterministic estimates; mock provider responses do not establish live prompt quality.

## Interactive compatibility checks

Device, desktop, credential, and live-provider tests are explicitly ignored in the default Rust suite. Their test annotations describe side effects. Run them individually on an interactive Windows desktop, with a disposable field and clipboard contents, after reading those annotations.

Check ordinary dictation in Notepad and browser text fields, including short answers, long passages, lists, and selected text. Verify:

- The ready sound follows actual microphone samples; an abandoned hold never starts recording later.
- Both modifier orders and right/left Ctrl/Win keys work, with repeated activation, hands-free stop, cancellation, and wake/unlock recovery.
- Repeated recording shortcuts during processing retain the existing job. Opening Hush preserves its result without pasting into the dashboard.
- Interrupted uploads, slow headers/bodies, provider errors, malformed responses, authentication failures, and rate limits end with a recoverable recording.
- Text arrives once, retains line breaks, and respects changed focus, protected inputs, and clipboard ownership.
- Notification text and recovery controls stay separate during expansion, at high DPI, and on side docks.
- Recording playback, WAV download, retry, linked deletion, restart recovery, and private-mode storage work in the packaged app.
- Command and whole-field prompt previews require approval and reject changed/unsupported fields.

Physical multi-monitor/mixed-DPI behavior, broad app compatibility, complete live dictation, packaged playback/retry, and prompt-edit acceptance remain incomplete. No passing result is inferred from a build or a mocked response.

## Distribution checks

`scripts/test-installer.ps1` checks a disposable per-user installation, installed background launch, uninstall, profile preservation, and isolation from unrelated Hush installations. Read its guards before running it on a development machine.

Release automation requires the machine-readable functional acceptance report and verified production signatures. Development builds are labeled unverified. Production SignPath enrollment and trusted signing are not active.

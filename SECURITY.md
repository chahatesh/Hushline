# Security policy

Hush 0.1.0 is pre-release software. No security audit or universal application-compatibility guarantee is claimed. Only the latest revision receives fixes.

Please report vulnerabilities privately using GitHub's private vulnerability reporting feature when enabled on the published repository. Otherwise contact the repository owner through their GitHub profile. Do not include an API key, private transcript, raw audio, or unrelated application content in a public issue.

## Security boundaries

- Groq is the exclusive inference provider; TLS requests go to `api.groq.com`. Audio is processed in the cloud.
- API keys are stored by Windows Credential Manager under `Hush/Groq`. They are returned only to native code, never to the dashboard. Any application running as the same Windows user may potentially access that user's generic credentials; this is not a defense against malware in the same account.
- Local transcript history, scratchpad, dictionary, and settings use SQLite in the current user's app-data directory. They are not database-encrypted. Protect your Windows account and disk; use private mode when appropriate.
- No telemetry, screenshots, surrounding-text capture, secret scraping, remote script loading, shell execution, or form submission is implemented.
- UI Automation checks password protection and focused element identity. Focus/caret changes cause recovery rather than a best-effort paste. Applications with inaccurate accessibility providers can still behave unexpectedly. Verify the result before sending or submitting anything.
- Automatic terminal insertion is blocked. Windows integrity boundaries are respected. Hush does not elevate to interact with administrator applications.
- Clipboard mode refuses unsupported snapshots and attempts a guarded restore after paste. A receiving app may consume clipboard content asynchronously; clipboard delivery cannot be universally verified. Unicode insertion is the default.
- Cancellation stops subsequent work; audio already uploaded cannot be recalled from Groq. Memory allocation contents are released, not forensically erased. API key buffers are zeroized where practical.
- No automatic updater is active. Install updates from a source you trust. Initial locally generated binaries are unsigned.

Dependency licenses remain with their respective owners. Microsoft compiler/SDK files used for local builds are development tools, not project source and not bundled into source distributions.

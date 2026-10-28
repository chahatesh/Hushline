# SignPath application preparation

**Draft facts only; not submitted and not approved.** The owner supplies contact/account information and agrees to the provider's terms directly.

| Field | Project information |
| --- | --- |
| Project | Hush (formerly Hushline) |
| Repository / homepage | https://github.com/chahatesh/Hushline |
| License | MIT; Copyright (c) 2026 Chahatesh |
| Maintainer / proposed reviewer and approver | Chahatesh, GitHub `chahatesh` |
| Function | Windows dictation with user-chosen Groq cloud recognition or local Whisper, formatting, dictionary/snippets, history and text insertion |
| Signable files | `hushline.exe`, generated `uninstall.exe`, `Hush_<version>_x64-setup.exe`; portable ZIP reuses the signed executable |
| Policy | https://github.com/chahatesh/Hushline/blob/main/CODE_SIGNING_POLICY.md |
| Privacy | https://github.com/chahatesh/Hushline/blob/main/PRIVACY.md |
| Build | GitHub-hosted Windows Actions; `.github/workflows/sign-windows.yml` |
| Existing distribution | Unsigned 0.6.0 installer/portable build, public CI artifacts; no official GitHub Release published at preparation time |
| Current status | Active development; functional live acceptance remains incomplete; production signing inactive |

Explain the GitHub artifact download process and provide a current public successful build run. Be candid about the project's age, user base and release status; do not invent reputation, release history or testing. The Foundation may require more maturity or a completed release before accepting it.

The application has no telemetry or subscription, uses a user-provided Groq key, and stores that key in Windows Credential Manager. Saved recording audio is encrypted with Windows DPAPI; private mode stays in memory. Cloud processing is visible and optional; offline mode has no cloud fallback. Microsoft WebView2 is an operating-system runtime dependency. Original dependency notices are included. No third-party binaries are submitted for signing under the project's identity.

Before activation, confirm MFA on GitHub and SignPath, review the signing policy and import the two constrained artifact configurations. These external conditions are not asserted as completed by this draft.

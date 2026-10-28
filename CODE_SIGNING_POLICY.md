# Code signing policy

**Status: integration prepared; production signing is not active.** The repository has no configured SignPath organization, signing policy or API token. No SignPath Foundation approval or production signature is claimed. Existing downloads remain unsigned.

The planned free provider is [SignPath Foundation](https://signpath.org/). Its program reviews eligibility and requires release approval. After enrollment, the attribution will be: Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

## Scope and maintainers

Only Hush's application executable, generated uninstaller and Windows installer, built from this public repository on GitHub-hosted runners, are submitted. Third-party plugins, offline engines, models and Microsoft components are not signed with Hush's project certificate. Original dependency licenses remain intact.

Committer, reviewer and signing approver: **[Chahatesh (@chahatesh)](https://github.com/chahatesh)**. Outside contributions require maintainer review. All signing participants must enable multi-factor authentication on GitHub and SignPath before activation; external account settings have not been verified by this repository.

The approver checks the source revision, workflow changes, build results and artifact version in SignPath. The workflow makes two requests: application/uninstaller, then the installer containing those signed files. Each requires the approval configured by the Foundation; approval is not bypassed or simulated.

## Build and verification

[Sign Windows distribution](.github/workflows/sign-windows.yml) builds and tests from source, uploads the two narrow signing inputs, and uses SignPath's GitHub connector to establish their origin. A second NSIS pass embeds the already signed application and uninstaller; it does not modify their bytes. The outer installer is then signed. The portable ZIP contains the same signed application.

Before distribution, checks require valid Windows Authenticode trust, the expected publisher, code-signing usage, a timestamp and matching signing identity for all three files. Returned executable contents must match their unsigned baselines apart from the certificate table, its directory, checksum and required alignment padding. Checksums accompany the final installer/ZIP. Unsigned, changed or test-signed output fails verification.

Manual signing builds produce artifacts without publishing a Release. The public release workflow retains the functional acceptance gate and creates a draft only after signed-output verification. A code signature identifies the publisher and protects integrity; it does not certify dictation accuracy or complete the outstanding live acceptance tests.

## Privacy and reporting

See [Privacy](PRIVACY.md). Signing submits build binaries and GitHub build metadata, never user recordings, transcript history or Groq API keys. The application sends dictation to Groq only when cloud recognition is selected and used; offline recognition has no cloud fallback. Optional AI processing and explicit editing requests are described in the privacy policy.

To inspect a download, use Windows **Properties → Digital Signatures**, or `Get-AuthenticodeSignature` in PowerShell. A signed build should show the approved SignPath Foundation publisher and a valid signature. Windows may still show reputation warnings for new signed builds; see [Microsoft's SmartScreen explanation](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation).

Report unexpected signatures or suspected compromise through [SECURITY.md](SECURITY.md). Do not attach keys, recordings or private text to a public report.

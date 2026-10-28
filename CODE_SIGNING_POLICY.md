# Code signing policy

**Current builds are unsigned.** SignPath Foundation enrollment and production signing are not active. The integration is prepared, but no trusted publisher identity is claimed.

## Scope

Hush's application executable, generated uninstaller, and installer are built from this public repository on GitHub-hosted runners. The signing workflow is designed to sign those files through [SignPath Foundation](https://signpath.org/). Third-party components retain their original licenses and signatures.

Maintainer, reviewer, and signing approver: [Chahatesh (@chahatesh)](https://github.com/chahatesh). Signing participants require multi-factor authentication. Outside contributions require maintainer review.

The workflow signs the application and uninstaller first, embeds their unchanged signed bytes, then signs the installer. Production distribution requires valid Windows Authenticode trust, the approved publisher, code-signing usage, a timestamp, matching identities, and unchanged executable contents. Test certificates do not satisfy these requirements. Portable ZIPs contain the same verified application; checksums accompany final artifacts.

## Verification and privacy

Inspect **Properties → Digital Signatures**, or use `Get-AuthenticodeSignature` in PowerShell. Signing verifies origin and integrity, not application accuracy. New signed builds may still receive [Windows SmartScreen reputation warnings](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation).

Signing submits build binaries and build metadata only. User recordings, transcripts, and API keys are excluded. See [Privacy](PRIVACY.md) and [Security](SECURITY.md).

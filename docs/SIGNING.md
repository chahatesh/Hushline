# Free Windows signing with SignPath Foundation

Status: **prepared; enrollment unverified and signing not configured**. No trusted certificate or signed Hush distribution is available yet. The existing 0.6.0 installer remains unsigned. Source code and local packaging checks cannot grant a publicly trusted publisher identity.

## Owner activation

1. Apply at **https://signpath.org/apply** using the facts in [SIGNPATH_APPLICATION.md](SIGNPATH_APPLICATION.md). The Foundation reviews eligibility, existing distributions and project reputation; approval is discretionary. Keep the status honest while approval is pending. Do not publish an unverified Release merely to satisfy enrollment requirements.
2. Complete the Foundation's account/project setup and MFA requirements. Link its GitHub trusted build system to `chahatesh/Hushline` and install the official SignPath GitHub App when directed. The service verifies artifact origin; local uploads are not a substitute for the trusted build workflow.
3. Import [windows-inner.xml](../signing/windows-inner.xml) as artifact configuration **windows-inner** and [windows-installer.xml](../signing/windows-installer.xml) as **windows-installer**. Both constrain Hush's product/version. Configure the approved production certificate and manual signing policy. Do not use a test certificate for distribution.
4. In this repository's Actions settings add secret **SIGNPATH_API_TOKEN**, limited to signing submission. Add variables **SIGNPATH_ORGANIZATION_ID**, **SIGNPATH_PROJECT_SLUG**, **SIGNPATH_SIGNING_POLICY_SLUG**, using the real values from SignPath. Never paste the token into source code, a command-line argument or this document.
5. Run **Actions → Sign Windows distribution → Run workflow**. Review the source/build, then approve its two SignPath requests. If approval does not arrive within 30 minutes per request, the workflow fails without an unsigned fallback. It produces signed build artifacts and checksums, without publishing a Release. Inspect the real signatures before changing the public signing status.
6. Official tagged releases additionally require `scripts/verify-release.mjs` to pass the live functional acceptance report. The release workflow consumes only verified signed artifacts and prepares a draft. Do not weaken that gate or mark unperformed checks as passed.

The owner must complete external enrollment and supply the resulting credentials. This work has not submitted an application, accepted provider terms on the owner's behalf, created a paid account or fabricated a certificate.

## Packaging sequence

The build first creates the ordinary installer and its generated NSIS script. `signpath.ps1 -Phase Prepare` exports the uninstaller and stages only `hushline.exe`/`uninstall.exe`. SignPath signs that GitHub artifact. `Repack` checks trust, timestamp, publisher, metadata and unchanged PE contents before embedding those files with NSIS's documented external-signing method. A second GitHub artifact supplies only the rebuilt installer for its outer signature. `Finalize` checks the same identity and byte integrity again, creates the signed portable ZIP, and writes checksums/provenance.

No application executable is modified after its signature is returned. NSIS plugins and other third-party components are not re-signed under Hush's certificate. New signed artifacts are separate from unsigned development builds. The ordinary CI artifact remains labeled unverified.

## Executed checks and limits

`scripts/test-windows-signing.ps1` exports real installer inputs, signs isolated copies with an in-memory **test-only** certificate, checks byte preservation/tamper rejection, verifies production gates reject the untrusted signer, and compiles the real NSIS signed-payload import path. It does not install a trusted root, persist a private key or alter the running app. Test-signed files stay in ignored test folders outside distribution upload globs.

This proves packaging/verification plumbing, not Foundation approval or a successful production signing request. Actual SignPath signing, trusted timestamps, installed signed payload verification and first-download SmartScreen behavior remain unverified until enrollment. Tests never label a self-signed certificate as trusted.

Official references: [Foundation terms](https://signpath.org/terms), [GitHub integration](https://docs.signpath.io/trusted-build-systems/github), [artifact configuration](https://docs.signpath.io/artifact-configuration/), [NSIS external uninstaller signing](https://nsis.sourceforge.io/Signing_an_Uninstaller_externally), and [SmartScreen reputation](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation). Signing changes publisher verification; it cannot promise that every Windows reputation warning disappears immediately.

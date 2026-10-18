# Publishing this repository

The workspace began empty; no pre-existing repository was replaced. The connected GitHub account is `chahatesh`, but GitHub CLI authentication/repository-creation access was unavailable during implementation. The connector exposes repository reading/editing but no repository-creation operation. No repository or release has been fabricated.

Once you choose to authenticate the GitHub CLI, run these exact commands from the repository root. Check that `chahatesh/hushline` is available first. If it exists, choose a new name; never force-push over it.

```powershell
gh auth login
gh repo view chahatesh/hushline
# Continue only if GitHub confirms that this repository does not exist.
gh repo create chahatesh/hushline --public --source . --remote origin --push --description "Independent MIT-licensed Windows dictation powered by Groq, Rust, and Tauri"
```

The lookup may fail for reasons other than nonexistence. Resolve authentication/network errors before creating anything. The initialized local repository contains meaningful commits. Git uses your configured identity and signing settings; no cryptographic signature is fabricated.

After publishing, replace README badge placeholders with the actual workflow and release URLs. Add the screenshots already in `docs/screenshots`, enable private vulnerability reporting, and review branch-protection rules. No secrets are required for CI or packaging.

## Release policy

1. Run the complete Windows/Groq manual acceptance protocol in QA.md.
2. Record actual results in release-verification.json, including tester, hardware, date, and supporting evidence. Do not turn failed/unexecuted checks into passes.
3. Run all automated checks and build both artifacts independently on a clean Windows setup.
4. Review the binary and installer, scan the distribution, and sign it with your own code-signing certificate if available. Hushline's MIT license does not make unsigned installers Authenticode-trusted.
5. Tag only a verified revision. The release workflow refuses incomplete acceptance evidence and builds before preparing a draft GitHub Release. Review that draft before publishing.

Example after verification:

```powershell
node scripts/verify-release.mjs
git tag -a v0.1.0 -m "Hushline 0.1.0"
git push origin main
git push origin v0.1.0
```

Use `git tag -s` only if your own signing key is configured. A configured commit-signing policy is preserved. No release is published automatically from an unverified local build.

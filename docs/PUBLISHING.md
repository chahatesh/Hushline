# Publishing this repository

The public repository is https://github.com/chahatesh/Hushline. `origin` points to it. The repository was newly created with the owner’s authorization; no existing repository was overwritten. GitHub CLI sign-in uses the owner’s own account.

After checks pass, commit and push from the root:

```powershell
git add .
git commit -m "Describe the verified change"
git push origin main
```

Use your own configured signing identity if signing is enabled. No fabricated signature is used.

## Release policy

Run the complete real Windows/Groq acceptance protocol in QA.md. Record actual evidence in release-verification.json; do not mark unexecuted checks as passing. Run checks and build on a clean second Windows machine. Sign installers only with an authorized certificate.

For 0.6.0, also verify the abandoned-hold/startup-ready sound, dark preparation state, release-to-insertion reliability, Casual/Standard/Formal formatting, correction/list examples, and whole-field prompt Clarify/Expand with edited preview, unchanged-target replacement, changed-target refusal, protected-field refusal, cancellation, and timeout. The prompt's 32 KB input/100 KB output limits and transient no-history behavior must remain intact. Browser tests and HTTP fixtures do not substitute for those live checks.

The release workflow blocks incomplete acceptance evidence. After acceptance passes, tag the verified revision and push the tag; the workflow builds artifacts and prepares a draft release for review. No unverified public release should be published.

Version 0.6.0 also requires live acceptance of recording history across restart, successful/failed/cancelled retry, packaged WebView2 playback and WAV download, linked deletion/Clear all, and no disk persistence in private mode or with recording storage disabled. Record these in `recordingHistory`, `recordingRetry`, `recordingPlaybackDownload` and `recordingDeletionPrivateMode`; automated fixtures alone do not satisfy those fields.

```powershell
node scripts/verify-release.mjs
git tag -a v0.6.0 -m "Hush 0.6.0"
git push origin main
git push origin v0.6.0
```

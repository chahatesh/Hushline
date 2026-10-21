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

The release workflow blocks incomplete acceptance evidence. After acceptance passes, tag the verified revision and push the tag; the workflow builds artifacts and prepares a draft release for review. No unverified public release should be published.

```powershell
node scripts/verify-release.mjs
git tag -a v0.3.3 -m "Hush 0.3.3"
git push origin main
git push origin v0.3.3
```

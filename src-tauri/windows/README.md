# Windows installer identity

`installer.nsi` derives from the MIT-licensed Tauri `tauri-cli-v2.12.3` template:
https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.3/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi

Upstream SHA-256: `8ae02c66f8adb4a0d15d97f984fbacd2b44262d8a4d03fefaf6177f67f7008e5`.
The Tauri contributors retain ownership; their complete MIT license is included in `TAURI-LICENSE-MIT`.

Only three substitutions are made, besides the attribution header:

- The uninstall registry key uses `${BUNDLEID}` instead of `${PRODUCTNAME}`.
- The manufacturer/product registry key uses `${BUNDLEID}` instead of `${PRODUCTNAME}`.
- Shortcuts use `Hush Dictation.lnk` instead of `Hush.lnk`.

This prevents collision with an unrelated older application already registered as Hush. No existing registration, shortcut, user profile or unrelated app is migrated or removed. Earlier Hushline installations can be removed normally through Windows after testing Hush; keep app data when prompted. The stable app-data identifier and credential slot preserve this project's settings and key.

When upgrading the Tauri CLI, compare the new upstream template, retain its license and reapply only these substitutions. Run the actual installer coexistence/uninstall check before shipping.

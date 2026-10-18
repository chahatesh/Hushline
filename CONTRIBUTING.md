# Contributing to Hushline

Thank you for helping make voice input more useful. Discuss large changes in an issue first. Small bug fixes, accessibility improvements, tests, and documentation are welcome.

Install Node.js 22 LTS, the stable Rust MSVC toolchain, Microsoft C++ Build Tools with the Windows SDK, and WebView2. See README.md for build steps. Run `npm ci`, `npm run build`, `npm run lint`, `npm test`, `npm run test:ui`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`, and `cargo test --manifest-path src-tauri/Cargo.toml` before submitting a pull request.

Keep shortcut callbacks free of audio, disk, UI Automation, and network operations. Do not add telemetry, alternative cloud inference providers, hidden content collection, key material in the frontend, or implicit execution of dictated commands. Fail closed on unknown input protection or changed targets. Add a regression test for changes to insertion, cancellation, privacy, or normalization. Preserve text meaning; ambiguous repairs belong in an explicit AI mode and should remain visible to users.

Use original code/assets, add `SPDX-License-Identifier: MIT` to original source where appropriate, and preserve third-party notices. Contributions to original project code are made under the MIT license. Do not copy proprietary implementations or product assets.

Use focused commits with descriptive messages. If your own Git signing identity is configured, signed commits are welcome. Do not impersonate another contributor or fabricate verification. Report exactly which Windows application/version and scenarios you tested.

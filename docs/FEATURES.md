# Feature status

**Implemented** means relevant automated or component verification exists. **Partially Implemented** means code exists with limitations or outstanding live acceptance. **Not Implemented** means absent. These statuses do not promise compatibility with every Windows app.

| Feature | Status | Scope |
| --- | --- | --- |
| Rust/Tauri/Solid desktop, original branding and MIT source | Implemented | Native build, interface tests, and installer checks. |
| Groq speech recognition, language selection and vocabulary hints | Partially Implemented | Documented requests and HTTP regressions; public speech fixture verified. Full live dictation acceptance remains open. |
| WASAPI capture and voice activity detection | Partially Implemented | Device frames, preparation/cancellation, and gap handling tested; broader device coverage pending. |
| Push-to-talk, hands-free, double-tap and custom shortcuts | Partially Implemented | Routing/lifecycle regressions; physical shortcut and resume/unlock acceptance pending. |
| Native voice bar and docking | Partially Implemented | Geometry and rendering tests; physical mixed-DPI/multi-monitor coverage pending. |
| System-wide insertion and clipboard restoration | Partially Implemented | Owned native-control and browser fixtures; full live speech delivery and broad app compatibility pending. |
| Protected-field, terminal, changed-focus and cancellation guards | Partially Implemented | Native/component regressions; broader accessibility-provider coverage pending. |
| Local cleanup, clear corrections, paragraphs and lists | Implemented | English bounded rules with fact/identifier/list regressions; ambiguous wording remains conservative. |
| Casual, Standard, Formal and independent AI frequency | Implemented | Settings migration, processing combinations, and interface tests. Stronger Formal rewriting needs AI. |
| Optional Groq AI cleanup and local fallback | Partially Implemented | HTTP and validation regressions plus synthetic live edits; general accuracy is not guaranteed. |
| Personal dictionary and exact-trigger snippets | Implemented | Persistence, import/export, replacements, whole-trigger expansion, and previews tested. |
| App-specific styles | Partially Implemented | Executable-name matching; browser-domain detection is absent. |
| Transcript history, scratchpad, statistics and backup | Implemented | SQLite and interface tests. Backups exclude keys, transcripts and audio. |
| Encrypted saved recording history and bounded retention | Partially Implemented | DPAPI restart, retention, linked deletion and privacy regressions; packaged live acceptance pending. |
| Listen, download and retry recognition | Partially Implemented | Binary WAV/HTTP/interface tests; packaged playback and live retry acceptance pending. |
| Command Mode and Improve prompt | Partially Implemented | Input limits, preview/cancellation/identity regressions; full live selected/whole-field acceptance pending. |
| Offline Whisper Tiny/Base | Partially Implemented | Verified downloads and real Tiny CPU speech fixture; Base accuracy and live delivery pending. |
| Local Ollama rewriting | Partially Implemented | Local-only client and small-model trial; general live editing acceptance pending. |
| Credential Manager API-key storage | Partially Implemented | Round-trip/native IPC tests; independent installation acceptance pending. |
| Per-user installer, portable ZIP and opt-in startup | Partially Implemented | Automated silent install/uninstall and packaging; interactive wizard/upgrade coverage pending. |
| No Hush account, app quota, subscription or telemetry | Implemented | No such service or restriction in the application. Groq limits still apply. |
| CI and source publication | Implemented | Public repository and Windows lint/test/build/package workflow. |
| Trusted signing and public GitHub Releases | Not Implemented | SignPath integration exists; external approval/credentials and functional acceptance remain pending. |
| Signed automatic updater | Not Implemented | No automatic application update service. |
| Rich-text snippets, browser-domain styles, long-session chunking | Not Implemented | Plain text, executable context and bounded whole recordings only. |

See [testing](QA.md), [privacy](../PRIVACY.md), and [offline setup](OFFLINE.md).

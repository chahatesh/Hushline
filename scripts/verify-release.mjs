// SPDX-License-Identifier: MIT
import fs from 'node:fs';
const report = JSON.parse(fs.readFileSync(new URL('../docs/release-verification.json', import.meta.url), 'utf8'));
const required = ['installLaunch', 'saveKey', 'selectMicrophone', 'pushToTalkNotepad', 'handsFree', 'cancel', 'focusChange', 'clipboardRestore', 'commandPreview', 'independentBuild'];
if (report.version !== '0.1.0' || !report.tester || !report.hardware || !report.testedAt || required.some(key => report.checks[key] !== true)) {
  throw new Error('Public release blocked: core Windows/Groq acceptance evidence is incomplete. Complete docs/release-verification.json with real results.');
}
console.log('Core acceptance evidence is present. Review the recorded test evidence before publishing.');

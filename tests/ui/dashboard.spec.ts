// SPDX-License-Identifier: MIT
// UI behavior only. These tests do not claim native/Groq end-to-end acceptance.
import { test, expect } from '@playwright/test';
test.beforeEach(async ({ page }) => { await page.goto('/'); await expect(page.getByRole('heading', { name: 'Let your thoughts flow.' })).toBeVisible(); });
test('empty workspace, original branding, and honest native boundary', async ({ page }) => {
  await expect(page.getByText('Your first words belong here')).toBeVisible();
  await expect(page.getByText('Browser preview', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'docs/screenshots/home.png', fullPage: true });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Save key', exact: true })).toBeDisabled();
  await expect(page.locator('input[type="password"]')).toHaveAttribute('autocomplete', 'off');
});
test('dictionary CRUD persists after reload and exports real data', async ({ page }) => {
  await page.getByRole('button', { name: 'Dictionary', exact: true }).click();
  await page.getByLabel('Word or phrase').fill('groak');
  await page.getByLabel('Replace with').fill('Groq');
  await page.getByRole('button', { name: 'Add word', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit groak' })).toBeVisible();
  await page.reload(); await page.getByRole('button', { name: 'Dictionary', exact: true }).click();
  await expect(page.getByText('→ Groq', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Edit groak' }).click(); await page.getByLabel('Replace with').fill('GroqCloud'); await page.getByRole('button', { name: 'Save word' }).click();
  await expect(page.getByText('→ GroqCloud', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'docs/screenshots/dictionary.png', fullPage: true });
  await page.getByRole('button', { name: 'Delete groak' }).click(); await expect(page.getByRole('button', { name: 'Edit groak' })).toHaveCount(0);
});
test('multiline snippets and writing styles are editable', async ({ page }) => {
  await page.getByRole('button', { name: 'Snippets', exact: true }).click(); await page.getByLabel('When I say').fill('my signature'); await page.getByLabel('Write this').fill('Best regards,\nChahatesh\n{{date}}'); await page.getByRole('button', { name: 'Add snippet' }).click();
  await expect(page.getByRole('button', { name: 'Edit my signature' })).toBeVisible();
  await page.getByRole('button', { name: 'Writing styles', exact: true }).click(); await page.getByRole('button', { name: 'Technical', exact: true }).click(); await page.getByLabel('Application match').fill('Code.exe'); await page.getByRole('button', { name: 'Add style' }).click(); await expect(page.getByRole('button', { name: 'Edit Technical' })).toBeVisible();
});
test('scratchpad saves and statistics do not fabricate usage', async ({ page }) => {
  await page.getByRole('button', { name: 'Scratchpad', exact: true }).click(); await page.getByLabel('Scratchpad', { exact: true }).fill('A quiet place\nfor a useful thought.'); await page.getByRole('button', { name: 'Save notes' }).click(); await page.reload(); await page.getByRole('button', { name: 'Scratchpad', exact: true }).click(); await expect(page.getByLabel('Scratchpad', { exact: true })).toHaveValue('A quiet place\nfor a useful thought.');
  await page.getByRole('button', { name: 'Statistics', exact: true }).click(); await expect(page.getByText('0 recordings in retained history')).toBeVisible();
});
test('settings, private mode, dark theme and backup exclusion', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click(); await page.getByLabel('Appearance').selectOption('dark'); await page.getByRole('tab', { name: 'Privacy & data' }).click(); await page.getByRole('switch', { name: /Save transcript history/ }).uncheck(); await page.getByRole('button', { name: 'Save settings' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  const downloadEvent = page.waitForEvent('download'); await page.getByText('Export configuration', { exact: true }).click(); const download = await downloadEvent; const stream = await download.createReadStream(); const chunks: Buffer[] = []; for await (const chunk of stream!) chunks.push(Buffer.from(chunk)); const backup = JSON.parse(Buffer.concat(chunks).toString()); expect(backup.settings.historyEnabled).toBe(false); expect(backup).not.toHaveProperty('history'); expect(backup).not.toHaveProperty('scratchpad'); expect(backup).not.toHaveProperty('apiKey');
  await page.screenshot({ path: 'docs/screenshots/settings-dark.png', fullPage: true });
});
test('keyboard navigation and compact viewport remain usable', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 700 }); await page.keyboard.press('Tab'); await expect(page.locator(':focus')).toBeVisible(); await page.getByRole('button', { name: 'About', exact: true }).click(); await expect(page.getByText('Chahatesh', { exact: true })).toBeVisible(); await expect(page.locator('body')).toHaveJSProperty('scrollWidth', 900);
});

test('offline selection, automatic layout, dock and paste preferences persist', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: /On this computer/ }).click();
  await expect(page.getByText('Open the Offline tab to download a speech pack.', { exact: false })).toBeVisible();
  await page.getByRole('tab', { name: 'Dictation', exact: true }).click();
  await expect(page.getByLabel('Text insertion', { exact: true })).toHaveValue('clipboard');
  await page.getByLabel('Automatic layout', { exact: true }).selectOption('notes');
  await page.getByRole('tab', { name: 'Voice bar', exact: true }).click();
  await page.getByRole('button', { name: 'Left side', exact: true }).click();
  await page.getByRole('button', { name: 'Preview active bar', exact: true }).click();
  await expect(page.locator('.pill-preview')).toHaveClass(/expanded/u);
  await page.evaluate(() => window.scrollTo(0, 0));
  await page.screenshot({ path: 'docs/screenshots/voice-bar.png', fullPage: true, animations: 'disabled' });
  await page.getByRole('tab', { name: 'Offline', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Download speech pack', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Save settings', exact: true }).click();
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('button', { name: /On this computer/ })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('tab', { name: 'Dictation', exact: true }).click();
  await expect(page.getByLabel('Automatic layout', { exact: true })).toHaveValue('notes');
  await page.getByRole('tab', { name: 'Voice bar', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Left side', exact: true })).toHaveAttribute('aria-pressed', 'true');
});

test('shortcut recorder and dedicated-key preset remain editable', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('tab', { name: 'Shortcuts', exact: true }).click();
  await page.getByRole('button', { name: 'Use dedicated-key shortcuts', exact: true }).click();
  await expect(page.getByLabel('Push-to-talk · hold', { exact: true })).toHaveValue('Ctrl+Shift+Space');
  await page.getByRole('button', { name: 'Record Push-to-talk · hold', exact: true }).click();
  await page.getByLabel('Push-to-talk · hold', { exact: true }).press('Control+Alt+R');
  await expect(page.getByLabel('Push-to-talk · hold', { exact: true })).toHaveValue('Ctrl+Alt+R');
  await page.getByRole('button', { name: 'Save settings', exact: true }).click();
  await page.reload();
  await expect(page.locator('.dictate-card .keys')).toContainText('R');
});

test('fast microphone preparation and pause behavior are explicit preferences', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('switch', { name: 'Fast microphone startup' })).toBeChecked();
  await page.getByRole('switch', { name: 'Fast microphone startup' }).uncheck();
  await page.getByRole('tab', { name: 'Dictation', exact: true }).click();
  await expect(page.getByLabel('Maximum recording (seconds)', { exact: true })).toHaveValue('600');
  await expect(page.getByRole('switch', { name: 'Stop hands-free recording when I go quiet' })).not.toBeChecked();
  await page.getByRole('switch', { name: 'Stop hands-free recording when I go quiet' }).check();
  await expect(page.getByLabel('Wait this many seconds', { exact: true })).toHaveValue('15');
  await page.getByRole('button', { name: 'Save settings', exact: true }).click();
  await page.reload();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('switch', { name: 'Fast microphone startup' })).not.toBeChecked();
  await page.getByRole('tab', { name: 'Dictation', exact: true }).click();
  await expect(page.getByLabel('Wait this many seconds', { exact: true })).toHaveValue('15');
});

test('Hush branding and cloud polish policy persist without ambiguity', async ({page}) => {
  await expect(page.locator('.brand').first()).toContainText('hush');
  await expect(page.locator('.brand').first()).not.toContainText('hushline');
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('tab',{name:'Dictation',exact:true}).click();
  const choices=page.getByRole('group',{name:'AI polish frequency'});
  await expect(choices.getByRole('button',{name:/Never/})).toHaveAttribute('aria-pressed','true');
  await choices.getByRole('button',{name:/Always/}).click();
  await page.getByRole('button',{name:'Save settings',exact:true}).click();
  await page.reload();
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('tab',{name:'Dictation',exact:true}).click();
  await expect(choices.getByRole('button',{name:/Always/})).toHaveAttribute('aria-pressed','true');
  await choices.getByRole('button',{name:/Sometimes/}).click();
  await page.getByText('Advanced transcription', {exact:true}).click();
  await page.getByRole('switch', {name:'Keep my exact words'}).check();
  await expect(choices.getByRole('button',{name:/Always/})).toBeDisabled();
  await expect(page.getByText('AI polish is paused while “Keep my exact words” is selected.')).toBeVisible();
  await page.getByRole('switch', {name:'Keep my exact words'}).uncheck();
  await expect(choices.getByRole('button',{name:/Sometimes/})).toHaveAttribute('aria-pressed','true');
  await page.locator('.polish-choice').screenshot({path:'docs/screenshots/cloud-polish.png',animations:'disabled'});
});

test('voice bar preview unfolds upward with an anchored base and respects reduced motion', async ({page}) => {
  await page.emulateMedia({reducedMotion:'reduce'});
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('tab',{name:'Voice bar',exact:true}).click();
  const pill=page.locator('.pill-preview');
  await page.getByRole('button',{name:'Preview active bar',exact:true}).scrollIntoViewIfNeeded();
  const rest=await pill.evaluate(e=>({y:e.offsetTop,height:e.offsetHeight}));
  await page.getByRole('button',{name:'Preview active bar',exact:true}).click();
  await expect(pill).toHaveCSS('height','32px');
  const active=await pill.evaluate(e=>({y:e.offsetTop,height:e.offsetHeight}));
  expect(active!.y).toBeLessThan(rest!.y);
  expect(Math.abs(active!.y+active!.height-rest!.y-rest!.height)).toBeLessThan(1);
  await expect(pill.getByText('Listening',{exact:true})).toHaveCount(0);
  await expect(pill.getByText('0:08',{exact:true})).toBeVisible();
  const positions=await pill.evaluate(e=>{const t=e.querySelector('small')!.getBoundingClientRect(), x=e.querySelector('svg')!.getBoundingClientRect(), w=e.querySelector('.preview-wave')!.getBoundingClientRect(); return {timer:t.x, close:x.x, wave:w.x+w.width/2, center:e.getBoundingClientRect().x+e.clientWidth/2};});
  expect(positions.timer).toBeLessThan(positions.close);
  expect(positions.timer).toBeLessThan(positions.wave);
  expect(Math.abs(positions.wave-positions.center)).toBeLessThan(1);
});


test('timeout preference persists and failed audio can be deleted from Home', async ({page}) => {
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('tab',{name:'Dictation',exact:true}).click();
  await expect(page.getByLabel('Transcription timeout (seconds)')).toHaveValue('30');
  await page.getByLabel('Transcription timeout (seconds)').fill('12');
  await page.getByRole('button',{name:'Save settings',exact:true}).click();
  await page.reload();
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('tab',{name:'Dictation',exact:true}).click();
  await expect(page.getByLabel('Transcription timeout (seconds)')).toHaveValue('12');
  await page.evaluate(()=>{const key='hushline-browser-preview-v1', data=JSON.parse(localStorage.getItem(key)!);data.savedAudio=[{id:'ui-metadata-only',createdAt:new Date().toISOString(),durationMs:8000,requiresReview:false,inMemory:true}];localStorage.setItem(key,JSON.stringify(data));});
  await page.reload();
  await expect(page.getByRole('heading',{name:/Recordings to retry/})).toBeVisible();
  await expect(page.getByText('Memory only · lost when Hush quits')).toBeVisible();
  await expect(page.getByRole('button',{name:'Retry transcription',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Delete saved audio'}).click();
  await page.getByRole('button',{name:'Delete permanently'}).click();
  await expect(page.getByRole('heading',{name:/Recordings to retry/})).toHaveCount(0);
});


test('writing style stays independent of AI frequency and exact-word override', async ({page}) => {
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Dictation',exact:true}).click();
  const styles=page.getByRole('group', {name:'Writing style',exact:true});
  const polish=page.getByRole('group', {name:'AI polish frequency',exact:true});
  await expect(styles.getByRole('button', {name:/Standard/})).toHaveAttribute('aria-pressed','true');
  await styles.getByRole('button', {name:/Casual/}).click();
  await expect(page.getByLabel('Automatic layout', {exact:true})).toBeDisabled();
  await expect(polish.getByRole('button', {name:/Never/})).toHaveAttribute('aria-pressed','true');
  await polish.getByRole('button', {name:/Always/}).click();
  await styles.getByRole('button', {name:/Formal/}).click();
  await expect(polish.getByRole('button', {name:/Always/})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByText('Its stronger grammar and wording improvements require AI polish below.', {exact:false})).toBeVisible();
  await page.getByText('Advanced transcription', {exact:true}).click();
  await page.getByRole('switch', {name:'Keep my exact words'}).check();
  await expect(styles.getByRole('button', {name:/Formal/})).toBeDisabled();
  await page.getByRole('button', {name:'Save settings',exact:true}).click();
  await page.reload();
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Dictation',exact:true}).click();
  await page.getByText('Advanced transcription', {exact:true}).click();
  await expect(page.getByRole('switch', {name:'Keep my exact words'})).toBeChecked();
  await page.getByRole('switch', {name:'Keep my exact words'}).uncheck();
  await expect(styles.getByRole('button', {name:/Formal/})).toHaveAttribute('aria-pressed','true');
  await expect(polish.getByRole('button', {name:/Always/})).toHaveAttribute('aria-pressed','true');
});

test('startup cue, preparation pause, and whole-field prompt action persist', async ({page}) => {
  await expect(page.getByText('Hush reads the whole field and previews an editable clarified version.', {exact:false})).toBeVisible();
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Dictation',exact:true}).click();
  await expect(page.getByLabel('Recording startup pause (milliseconds)')).toHaveValue('300');
  await page.getByLabel('Recording startup pause (milliseconds)').fill('500');
  await expect(page.getByRole('switch', {name:'Play a sound when recording starts'})).toBeChecked();
  await page.getByRole('switch', {name:'Play a sound when recording starts'}).uncheck();
  await page.getByRole('group', {name:'Prompt improvement action'}).getByRole('button', {name:/Expand/}).click();
  await page.getByRole('tab', {name:'Shortcuts',exact:true}).click();
  await expect(page.getByLabel('Improve prompt · whole field', {exact:true})).toHaveValue('Ctrl+Win+P');
  await page.getByLabel('Improve prompt · whole field', {exact:true}).fill('Ctrl+Shift+P');
  await page.getByRole('button', {name:'Save settings',exact:true}).click();
  await page.reload();
  await expect(page.locator('.prompt-help .keys')).toContainText('Shift');
  await expect(page.getByText('Hush reads the whole field and previews an editable expanded version.', {exact:false})).toBeVisible();
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Dictation',exact:true}).click();
  await expect(page.getByLabel('Recording startup pause (milliseconds)')).toHaveValue('500');
  await expect(page.getByRole('switch', {name:'Play a sound when recording starts'})).not.toBeChecked();
  await expect(page.getByRole('group', {name:'Prompt improvement action'}).getByRole('button', {name:/Expand/})).toHaveAttribute('aria-pressed','true');
});

test('side dock preview places timer above waveform and cancel below it', async ({page}) => {
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Voice bar',exact:true}).click();
  await page.getByRole('button', {name:'Right side',exact:true}).click();
  await page.getByRole('button', {name:'Preview active bar',exact:true}).click();
  const pill=page.locator('.pill-preview');
  await expect(pill).toHaveCSS('height','128px');
  await page.waitForTimeout(200);
  const positions=await pill.evaluate(e=>{const t=e.querySelector('small')!.getBoundingClientRect(),x=e.querySelector('svg')!.getBoundingClientRect(),w=e.querySelector('.preview-wave')!.getBoundingClientRect(),body=e.getBoundingClientRect(),dock=e.parentElement!.getBoundingClientRect();return {timer:t.bottom,waveTop:w.top,waveBottom:w.bottom,close:x.top,center:body.top+body.height/2,dockCenter:dock.top+dock.height/2};});
  expect(positions.timer).toBeLessThan(positions.waveTop);
  expect(positions.close).toBeGreaterThan(positions.waveBottom);
  expect(Math.abs(positions.center-positions.dockCenter)).toBeLessThan(1);
});

test('old polished profile upgrades without losing double tap or AI policy', async ({page}) => {
  await page.evaluate(()=>{localStorage.setItem('hushline-browser-preview-v1',JSON.stringify({settings:{preferencesVersion:4,mode:'polished',polishPolicy:'sometimes',doubleTap:true,historyEnabled:false,transcriptionTimeoutSeconds:18,shortcuts:{push:'Ctrl+Alt+R'}}}));});
  await page.reload();
  await expect(page.locator('.dictate-card .keys')).toContainText('R');
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Dictation',exact:true}).click();
  await expect(page.getByRole('group', {name:'Writing style',exact:true}).getByRole('button', {name:/Formal/})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByRole('group', {name:'AI polish frequency'}).getByRole('button', {name:/Sometimes/})).toHaveAttribute('aria-pressed','true');
  await expect(page.getByRole('switch', {name:'Double-tap for hands-free'})).toBeChecked();
  await expect(page.getByLabel('Transcription timeout (seconds)')).toHaveValue('18');
  await page.getByRole('tab', {name:'Shortcuts',exact:true}).click();
  await expect(page.getByLabel('Improve prompt · whole field', {exact:true})).toHaveValue('Ctrl+Win+P');
});

test('recording history shows linked audio and failed recordings without simulating native playback', async ({page}) => {
  await page.evaluate(() => localStorage.setItem('hushline-browser-preview-v1', JSON.stringify({
    history: [
      {id:'result',audioId:'public-recording',text:'Pack the camera and charge the batteries.',createdAt:'2026-10-27T16:00:00Z',app:'Notepad.exe',words:8,durationMs:4000,latencyMs:300,inserted:true},
      {id:'legacy',text:'An older note without recorded audio.',createdAt:'2026-10-26T16:00:00Z',app:'Notepad.exe',words:7,durationMs:3000,latencyMs:200,inserted:true},
    ],
    savedAudio: [
      {id:'public-recording',transcriptId:'result',createdAt:'2026-10-27T16:00:00Z',durationMs:4000,inMemory:false,requiresReview:false,status:'ready',attempts:1},
      {id:'failed-recording',createdAt:'2026-10-27T16:05:00Z',durationMs:7000,inMemory:false,requiresReview:false,status:'failed',attempts:2,lastError:'The connection failed. Check your network, then retry.'},
    ],
  })));
  await page.reload(); await page.getByRole('navigation', {name:'Main navigation'}).getByRole('button', {name:/^History/u}).click();
  const linked = page.locator('.transcript-row').filter({hasText:'Pack the camera'});
  await expect(linked.getByRole('button', {name:'Listen',exact:true})).toBeVisible();
  await expect(linked.getByRole('button', {name:'Download recording',exact:true})).toBeDisabled();
  await expect(linked.getByRole('button', {name:'Retry transcription',exact:true})).toBeDisabled();
  await expect(page.locator('.transcript-row').filter({hasText:'An older note'}).locator('.recording-actions')).toHaveCount(0);
  await expect(page.locator('.saved-audio')).toContainText('The connection failed.');
  await expect(page.locator('.saved-audio')).toContainText('2 attempts');
  await expect(page.locator('.saved-audio')).toContainText('Your recording stays available after a retry.');
  await page.screenshot({path:'output/playwright/recordings-0.6.0.png',fullPage:true});
  await page.setViewportSize({width:900,height:700});
  await expect(page.locator('body')).toHaveJSProperty('scrollWidth', 900);
  await linked.getByRole('button', {name:'Delete transcript',exact:true}).click();
  await expect(page.getByRole('alertdialog')).toContainText('recording');
  await page.getByRole('button', {name:'Delete permanently',exact:true}).click();
  const state = await page.evaluate(() => JSON.parse(localStorage.getItem('hushline-browser-preview-v1')!));
  expect(state.savedAudio.map((clip: {id:string}) => clip.id)).toEqual(['failed-recording']);
});

test('clear history removes recordings even when no transcript was returned', async ({page}) => {
  await page.evaluate(() => localStorage.setItem('hushline-browser-preview-v1', JSON.stringify({savedAudio:[{id:'private',createdAt:'2026-10-27T16:00:00Z',durationMs:1000,inMemory:true,status:'failed',attempts:1,requiresReview:false}]})));
  await page.reload(); await page.getByRole('button', {name:'History',exact:true}).click();
  await expect(page.getByRole('button', {name:'Clear all',exact:true})).toBeEnabled();
  await page.getByRole('button', {name:'Clear all',exact:true}).click();
  await page.getByRole('button', {name:'Delete permanently',exact:true}).click();
  await expect(page.locator('.saved-audio')).toHaveCount(0);
  const state = await page.evaluate(() => JSON.parse(localStorage.getItem('hushline-browser-preview-v1')!));
  expect(state.savedAudio).toEqual([]);
});

test('recording storage preference persists and configuration excludes recordings', async ({page}) => {
  await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Dictation',exact:true}).click();
  await expect(page.getByLabel('Transcription timeout (seconds)')).toHaveValue('30');
  await page.getByRole('tab', {name:'Privacy & data',exact:true}).click();
  await expect(page.getByRole('switch', {name:/^Save recording audio with history/u})).toBeChecked();
  await page.getByRole('switch', {name:/^Save recording audio with history/u}).uncheck();
  await page.getByRole('button', {name:'Save settings',exact:true}).click();
  await page.reload(); await page.getByRole('button', {name:'Settings',exact:true}).click();
  await page.getByRole('tab', {name:'Privacy & data',exact:true}).click();
  await expect(page.getByRole('switch', {name:/^Save recording audio with history/u})).not.toBeChecked();
  const downloadEvent = page.waitForEvent('download');
  await page.getByText('Export configuration', {exact:true}).click();
  const file = await downloadEvent;
  const stream = await file.createReadStream(); const chunks: Buffer[] = [];
  for await (const chunk of stream!) chunks.push(Buffer.from(chunk));
  const backup = JSON.parse(Buffer.concat(chunks).toString());
  expect(backup).not.toHaveProperty('savedAudio'); expect(backup).not.toHaveProperty('history');
});

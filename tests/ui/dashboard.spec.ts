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
  await page.getByLabel('Clean up my speech',{exact:true}).selectOption('literal');
  await expect(choices.getByRole('button',{name:/Always/})).toBeDisabled();
  await expect(page.getByText('AI polish is paused while “Keep my exact words” is selected.')).toBeVisible();
  await page.getByLabel('Clean up my speech',{exact:true}).selectOption('clean');
  await expect(choices.getByRole('button',{name:/Sometimes/})).toHaveAttribute('aria-pressed','true');
  await page.locator('.polish-choice').screenshot({path:'docs/screenshots/cloud-polish.png',animations:'disabled'});
});

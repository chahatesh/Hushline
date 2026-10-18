// SPDX-License-Identifier: MIT
// UI behavior only. These tests do not claim native/Groq end-to-end acceptance.
import { test, expect } from '@playwright/test';
test.beforeEach(async ({ page }) => { await page.goto('/'); await page.evaluate(() => localStorage.clear()); await page.reload(); await expect(page.getByRole('heading', { name: 'Let your thoughts flow.' })).toBeVisible(); });
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

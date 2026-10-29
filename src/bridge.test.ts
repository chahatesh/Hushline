// SPDX-License-Identifier: MIT
import { describe, it, expect } from 'vitest';
import { words, shortcutParts, keyboardChord } from './bridge';
import { defaults, migrateSettings } from './types';
describe('UI data contracts', () => {
  it('upgrades the old prompt default without replacing custom or occupied shortcuts', () => {
    const upgraded = migrateSettings({...defaults,preferencesVersion:6,shortcuts:{...defaults.shortcuts,improvePrompt:'Win+Ctrl+P'}});
    expect(upgraded.shortcuts.improvePrompt).toBe('Ctrl+Shift+P');
    const custom = migrateSettings({...defaults,preferencesVersion:6,shortcuts:{...defaults.shortcuts,improvePrompt:'Ctrl+Alt+R'}});
    expect(custom.shortcuts.improvePrompt).toBe('Ctrl+Alt+R');
    const occupied = migrateSettings({...defaults,preferencesVersion:6,shortcuts:{...defaults.shortcuts,improvePrompt:'Ctrl+Win+P',open:'Shift+Ctrl+P'}});
    expect(occupied.shortcuts.open).toBe('Shift+Ctrl+P');
    expect(occupied.shortcuts.improvePrompt).toBe('Ctrl+Win+P');
  });
  it('enables recording history and replaces only the old short default on upgrade', () => {
    expect(defaults.saveRecordings).toBe(true);
    expect(defaults.transcriptionTimeoutSeconds).toBe(30);
    expect(migrateSettings({ preferencesVersion: 5, writingStyle: 'casual', historyEnabled: false, transcriptionTimeoutSeconds: 5 })).toMatchObject({ preferencesVersion: 7, writingStyle: 'casual', historyEnabled: false, saveRecordings: true, transcriptionTimeoutSeconds: 30 });
    expect(migrateSettings({ preferencesVersion: 5, transcriptionTimeoutSeconds: 18, saveRecordings: false })).toMatchObject({ transcriptionTimeoutSeconds: 18, saveRecordings: false });
    expect(migrateSettings({ preferencesVersion: 6, transcriptionTimeoutSeconds: 5 })).toMatchObject({ transcriptionTimeoutSeconds: 5 });
  });
  it('counts words without counting whitespace', () => { expect(words(' \n\t ')).toBe(0); expect(words('One\n two   three')).toBe(3); });
  it('keeps modifier-only shortcuts readable', () => { expect(shortcutParts('Ctrl+Win')).toEqual(['Ctrl', 'Win']); expect(shortcutParts('Ctrl+Win+Space')).toEqual(['Ctrl', 'Win', 'Space']); });
  it('defaults to the path without an extra AI call', () => { expect(defaults.mode).toBe('clean'); expect(defaults.enhanced).toBe(false); expect(defaults.contextEnabled).toBe(false); expect(defaults.selectionPermission).toBe(false); });
  it('keeps keys outside settings and backup shapes', () => { expect(JSON.stringify(defaults).toLowerCase()).not.toContain('apikey'); expect(defaults.insertion).toBe('clipboard'); });
  it('records a complete chord without confusing Space and modifiers', () => {
    expect(keyboardChord({ ctrlKey:true, metaKey:false, altKey:false, shiftKey:true, code:'Space', key:' ' })).toBe('Ctrl+Shift+Space');
    expect(keyboardChord({ ctrlKey:true, metaKey:true, altKey:false, shiftKey:false, code:'MetaLeft', key:'Meta' })).toBe('Ctrl+Win');
  });
  it('keeps preparation feedback and prompt editing explicit', () => {
    expect(defaults.preferencesVersion).toBe(7);
    expect(defaults.startupDelayMs).toBe(300);
    expect(defaults.recordingCueEnabled).toBe(true);
    expect(defaults.writingStyle).toBe('standard');
    expect(defaults.promptImproveMode).toBe('clarify');
    expect(defaults.shortcuts.improvePrompt).toBe('Ctrl+Shift+P');
  });
  it('migrates old polish profiles without resetting independent choices', () => {
    const settings = migrateSettings({ ...defaults, preferencesVersion: 4, writingStyle: undefined, mode: 'polished', polishPolicy: 'sometimes', doubleTap: true, provider: 'offline', transcriptionTimeoutSeconds: 12 });
    expect(settings).toMatchObject({ preferencesVersion: 7, mode: 'clean', writingStyle: 'formal', polishPolicy: 'sometimes', doubleTap: true, provider: 'offline', transcriptionTimeoutSeconds: 12 });
    expect(settings.shortcuts.improvePrompt).toBe('Ctrl+Shift+P');
  });
  it('retains exact-word behavior and user-selected zero preparation delay', () => {
    const settings = migrateSettings({ preferencesVersion: 4, mode: 'literal', startupDelayMs: 0, recordingCueEnabled: false });
    expect(settings).toMatchObject({ mode: 'literal', writingStyle: 'casual', startupDelayMs: 0, recordingCueEnabled: false });
    expect(migrateSettings({ ...defaults, writingStyle: 'casual', polishPolicy: 'always' })).toMatchObject({ writingStyle: 'casual', polishPolicy: 'always' });
  });
  it('preserves an old custom shortcut when adding Improve prompt', () => {
    const settings = migrateSettings({ preferencesVersion: 4, shortcuts: { ...defaults.shortcuts, open: 'Ctrl+Shift+P' } });
    expect(settings.shortcuts.open).toBe('Ctrl+Shift+P');
    expect(settings.shortcuts.improvePrompt).toBe('Ctrl+Win+Shift+P');
    expect(defaults.shortcuts.improvePrompt).toBe('Ctrl+Shift+P');
  });
  it('compares shortcut modifier order and case before choosing a free fallback', () => {
    const shortcuts = { ...defaults.shortcuts, open: ' shift + CTRL + p ', command: 'win+ctrl+p', paste: 'SHIFT+Win+ctrl+P', copy: 'WIN+CTRL+f01', scratchpad: 'ctrl+win+F2' };
    const settings = migrateSettings({ preferencesVersion: 4, shortcuts });
    expect(settings.shortcuts.improvePrompt).toBe('Ctrl+Win+F3');
    for (const [key, value] of Object.entries(shortcuts).filter(([key]) => key !== 'improvePrompt')) expect(settings.shortcuts[key as keyof typeof shortcuts]).toBe(value);
    expect(migrateSettings({ preferencesVersion: 5, shortcuts }).shortcuts.improvePrompt).toBe('Ctrl+Win+F3');
  });
});

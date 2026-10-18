// SPDX-License-Identifier: MIT
import { describe, it, expect } from 'vitest';
import { words, shortcutParts } from './bridge';
import { defaults } from './types';
describe('UI data contracts', () => {
  it('counts words without counting whitespace', () => { expect(words(' \n\t ')).toBe(0); expect(words('One\n two   three')).toBe(3); });
  it('keeps modifier-only shortcuts readable', () => { expect(shortcutParts('Ctrl+Win')).toEqual(['Ctrl', 'Win']); expect(shortcutParts('Ctrl+Win+Space')).toEqual(['Ctrl', 'Win', 'Space']); });
  it('defaults to the path without an extra AI call', () => { expect(defaults.mode).toBe('clean'); expect(defaults.enhanced).toBe(false); expect(defaults.contextEnabled).toBe(false); expect(defaults.selectionPermission).toBe(false); });
  it('keeps keys outside settings and backup shapes', () => { expect(JSON.stringify(defaults).toLowerCase()).not.toContain('apikey'); expect(defaults.insertion).toBe('unicode'); });
});

// SPDX-License-Identifier: MIT
import { invoke, isTauri } from '@tauri-apps/api/core';
import { defaults, emptyView, migrateSettings, type Snapshot, type Backup, type Settings } from './types';
export const desktop = isTauri();
const previewKey = 'hushline-browser-preview-v1';
function initial(): Snapshot { return { offline: { installed: [], engineVersion: '1.9.2' }, settings: structuredClone(defaults), dictionary: [], snippets: [], styles: [], history: [], savedAudio: [], diagnostics: [], scratchpad: '', view: { ...emptyView }, startupEnabled: false }; }
function readPreview(): Snapshot { try { const stored = localStorage.getItem(previewKey); if (stored) { const parsed = JSON.parse(stored); return { ...initial(), ...parsed, settings: migrateSettings(parsed.settings), view: { ...emptyView } }; } } catch { /* A corrupt preview never affects native storage. */ } return initial(); }
export async function call<T = void>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  if (desktop) return invoke<T>(command, args);
  // Browser-only UI review: no microphone, keys, cloud calls, or system input are simulated.
  const data = readPreview();
  let result: unknown;
  switch (command) {
    case 'snapshot': result = data; break;
    case 'save_settings': data.settings = args.settings as Settings; break;
    case 'save_dictionary': data.dictionary = args.entries as Snapshot['dictionary']; break;
    case 'save_snippets': data.snippets = args.entries as Snapshot['snippets']; break;
    case 'save_styles': data.styles = args.entries as Snapshot['styles']; break;
    case 'save_scratchpad': data.scratchpad = String(args.content); break;
    case 'microphones': result = []; break;
    case 'set_shortcut_capture': break;
    case 'delete_saved_audio': data.savedAudio = data.savedAudio.filter(a => a.id !== args.id); break;
    case 'delete_history': data.history = args.id ? data.history.filter(t => t.id !== args.id) : []; break;
    case 'export_configuration': result = { version: 1, settings: data.settings, dictionary: data.dictionary, snippets: data.snippets, styles: data.styles } satisfies Backup; break;
    case 'import_configuration': {
      const b = JSON.parse(String(args.content)) as Backup;
      if (b.version !== 1 || !b.settings || !Array.isArray(b.dictionary) || !Array.isArray(b.snippets) || !Array.isArray(b.styles)) throw new Error('Invalid backup file.');
      Object.assign(data, b, { settings: migrateSettings(b.settings) }); break;
    }
    case 'copy_text': await navigator.clipboard.writeText(String(args.text)); break;
    default: throw new Error('Open the installed Windows app to use microphone, secure key storage, and system-wide dictation.');
  }
  localStorage.setItem(previewKey, JSON.stringify(data));
  return result as T;
}
export function download(name: string, data: string, type = 'application/json') { const url = URL.createObjectURL(new Blob([data], { type })); const a = document.createElement('a'); a.href = url; a.download = name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); }
export function errorText(error: unknown) { return error instanceof Error ? error.message : String(error); }
export function words(text: string) { return text.trim() ? text.trim().split(/\s+/u).length : 0; }
export function shortcutParts(shortcut: string) { return shortcut.split('+').map(s => s === 'Win' ? 'Win' : s === 'Space' ? 'Space' : s); }
export function keyboardChord(e: Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey' | 'code' | 'key'>) {
  const parts = [e.ctrlKey && 'Ctrl', e.metaKey && 'Win', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean) as string[];
  if (/^Key[A-Z]$/u.test(e.code)) parts.push(e.code.slice(3));
  else if (/^Digit[0-9]$/u.test(e.code)) parts.push(e.code.slice(5));
  else if (/^F(?:[1-9]|1[0-9]|2[0-4])$/u.test(e.key)) parts.push(e.key);
  else if (e.code === 'Space') parts.push('Space');
  else if (e.code === 'Escape') parts.push('Escape');
  return parts.join('+');
}

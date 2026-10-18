// SPDX-License-Identifier: MIT
export type Mode = 'literal' | 'clean' | 'polished';
export interface Shortcuts { push: string; handsFree: string; command: string; paste: string; copy: string; open: string; scratchpad: string; cancel: string }
export interface Settings { microphone: string; speechModel: string; cleanupModel: string; commandModel: string; language: string; mode: Mode; enhanced: boolean; historyEnabled: boolean; contextEnabled: boolean; selectionPermission: boolean; maxSeconds: number; silenceSeconds: number; vadThreshold: number; doubleTap: boolean; theme: string; insertion: string; shortcuts: Shortcuts }
export interface Entry { id: string; source: string; replacement: string }
export interface Snippet { id: string; trigger: string; content: string; confirm: boolean }
export interface Style { id: string; name: string; appMatch: string; instruction: string; enhanced: boolean }
export interface Transcript { id: string; text: string; createdAt: string; app: string; words: number; durationMs: number; latencyMs: number; inserted: boolean }
export interface View { phase: string; message: string; level: number; seconds: number; lastText: string; preview: string | null; original: string | null; hasKey: boolean; generation: number; warning: string }
export interface Snapshot { settings: Settings; dictionary: Entry[]; snippets: Snippet[]; styles: Style[]; history: Transcript[]; scratchpad: string; view: View; startupEnabled: boolean }
export interface Backup { version: number; settings: Settings; dictionary: Entry[]; snippets: Snippet[]; styles: Style[] }
export const defaults: Settings = { microphone: '', speechModel: 'whisper-large-v3-turbo', cleanupModel: 'openai/gpt-oss-20b', commandModel: 'openai/gpt-oss-120b', language: '', mode: 'clean', enhanced: false, historyEnabled: true, contextEnabled: false, selectionPermission: false, maxSeconds: 120, silenceSeconds: 8, vadThreshold: .012, doubleTap: false, theme: 'system', insertion: 'unicode', shortcuts: { push: 'Ctrl+Win', handsFree: 'Ctrl+Win+Space', command: 'Ctrl+Win+Alt', paste: 'Ctrl+Win+V', copy: 'Ctrl+Win+C', open: 'Ctrl+Win+H', scratchpad: 'Ctrl+Win+J', cancel: 'Escape' } };
export const emptyView: View = { phase: 'idle', message: '', level: 0, seconds: 0, lastText: '', preview: null, original: null, hasKey: false, generation: 0, warning: '' };

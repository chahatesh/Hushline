// SPDX-License-Identifier: MIT
export type Mode = 'literal' | 'clean' | 'polished';
export type WritingStyle = 'casual' | 'standard' | 'formal';
export type PromptImproveMode = 'clarify' | 'expand';
export interface Shortcuts { push: string; handsFree: string; command: string; improvePrompt: string; paste: string; copy: string; open: string; scratchpad: string; cancel: string }
export interface Settings { preferencesVersion: number; provider: 'groq' | 'offline'; offlineModel: 'tiny' | 'base'; localTextModel: string; formatting: 'auto' | 'paragraphs' | 'notes' | 'off'; dock: 'bottom' | 'left' | 'right' | 'hidden'; microphone: string; prepareMicrophone: boolean; startupDelayMs: number; recordingCueEnabled: boolean; speechModel: string; cleanupModel: string; commandModel: string; language: string; mode: Mode; writingStyle: WritingStyle; promptImproveMode: PromptImproveMode; enhanced: boolean; polishPolicy: 'always' | 'sometimes' | 'never'; historyEnabled: boolean; saveRecordings: boolean; contextEnabled: boolean; selectionPermission: boolean; maxSeconds: number; transcriptionTimeoutSeconds: number; silenceSeconds: number; vadThreshold: number; doubleTap: boolean; theme: string; insertion: string; shortcuts: Shortcuts }
export interface Entry { id: string; source: string; replacement: string }
export interface Snippet { id: string; trigger: string; content: string; confirm: boolean }
export interface Style { id: string; name: string; appMatch: string; instruction: string; enhanced: boolean }
export interface Transcript { id: string; text: string; createdAt: string; app: string; words: number; durationMs: number; latencyMs: number; inserted: boolean; deliveryFailure?: string; audioId?: string }
export interface View { phase: string; stage: string; message: string; level: number; seconds: number; lastText: string; recoverable: boolean; preview: string | null; previewKind: string; previewId: string | null; original: string | null; promptGuidance: string; hasKey: boolean; generation: number; warning: string }
export interface OfflineStatus { installed: string[]; engineVersion: string }
export interface DownloadProgress { stage: string; received: number; total: number }
export interface SavedAudio { id: string; createdAt: string; durationMs: number; requiresReview: boolean; inMemory: boolean; status: 'pending' | 'ready' | 'failed'; transcriptId?: string | null; lastError?: string | null; attempts: number }
export interface StageDiagnostic { generation: number; stage: string; elapsedMs: number; outcome: string }
export interface HistorySummary { count: number; words: number; revision: number }
export interface HistoryPage { records: Transcript[]; total: number; offset: number; revision: number }
export interface Usage { words: number; recordings: number; durationMs: number; latencyMs: number; inserted: number; longest: number; withLists: number; withParagraphs: number }
export interface NamedUsage extends Usage { name: string }
export interface Signal { name: string; count: number }
export interface VoiceProfile { fromWord: number; throughWord: number; completedAt: string; topics: Signal[]; terms: Signal[] }
export interface Statistics { total: Usage; days: NamedUsage[]; apps: NamedUsage[]; hours: Usage[]; weekdays: Usage[]; profiles: VoiceProfile[]; topics: Signal[]; terms: Signal[]; unclassifiedWords: number; skippedRecords: number }
export interface Snapshot { historySummary: HistorySummary; offline: OfflineStatus; settings: Settings; dictionary: Entry[]; snippets: Snippet[]; styles: Style[]; history: Transcript[]; savedAudio: SavedAudio[]; diagnostics: StageDiagnostic[]; scratchpad: string; view: View; startupEnabled: boolean }
export interface Backup { version: number; settings: Settings; dictionary: Entry[]; snippets: Snippet[]; styles: Style[] }
export const defaults: Settings = { preferencesVersion: 7, provider: 'groq', offlineModel: 'tiny', localTextModel: '', formatting: 'auto', dock: 'bottom', microphone: '', prepareMicrophone: true, startupDelayMs: 300, recordingCueEnabled: true, speechModel: 'whisper-large-v3-turbo', cleanupModel: 'openai/gpt-oss-20b', commandModel: 'openai/gpt-oss-120b', language: '', mode: 'clean', writingStyle: 'standard', promptImproveMode: 'clarify', enhanced: false, polishPolicy: 'never', historyEnabled: true, saveRecordings: true, contextEnabled: false, selectionPermission: false, maxSeconds: 600, transcriptionTimeoutSeconds: 30, silenceSeconds: 0, vadThreshold: .012, doubleTap: false, theme: 'system', insertion: 'clipboard', shortcuts: { push: 'Ctrl+Win', handsFree: 'Ctrl+Win+Space', command: 'Ctrl+Win+Alt', improvePrompt: 'Ctrl+Shift+P', paste: 'Ctrl+Win+V', copy: 'Ctrl+Win+C', open: 'Ctrl+Win+H', scratchpad: 'Ctrl+Win+J', cancel: 'Escape' } };
export const emptyView: View = { phase: 'idle', stage: '', message: '', level: 0, seconds: 0, lastText: '', recoverable: false, preview: null, previewKind: '', previewId: null, original: null, promptGuidance: '', hasKey: false, generation: 0, warning: '' };

function shortcutIdentity(value: string) {
  const parts = value.split('+').map(part => part.trim().toUpperCase());
  const modifiers = parts.filter(part => ['CTRL', 'WIN', 'ALT', 'SHIFT'].includes(part)).sort();
  const keys = parts.filter(part => !['CTRL', 'WIN', 'ALT', 'SHIFT'].includes(part)).map(part => /^F\d+$/u.test(part) ? `F${Number(part.slice(1))}` : part);
  return [...modifiers, ...keys].join('+');
}

/** Mirror the desktop migration for browser reviews and old configuration imports. */
export function migrateSettings(stored: Partial<Settings> = {}): Settings {
  const settings: Settings = { ...defaults, ...stored, shortcuts: { ...defaults.shortcuts, ...stored.shortcuts } };
  if ((stored.preferencesVersion ?? 0) < 5) {
    settings.writingStyle = stored.writingStyle ?? (stored.mode === 'polished' ? 'formal' : stored.mode === 'literal' ? 'casual' : 'standard');
    settings.mode = stored.mode === 'literal' ? 'literal' : 'clean';
    const occupied = new Set(Object.entries(settings.shortcuts).filter(([action]) => action !== 'improvePrompt').map(([, value]) => shortcutIdentity(value)));
    if (occupied.has(shortcutIdentity(settings.shortcuts.improvePrompt))) {
      const available = ['Ctrl+Win+Shift+P', ...Array.from({ length: 24 }, (_, index) => `Ctrl+Win+F${index + 1}`)].find(chord => !occupied.has(shortcutIdentity(chord)));
      if (available) settings.shortcuts.improvePrompt = available;
    }
    settings.preferencesVersion = 5;
  }
  if ((stored.preferencesVersion ?? 0) < 6) {
    if (settings.transcriptionTimeoutSeconds === 5) settings.transcriptionTimeoutSeconds = 30;
    settings.preferencesVersion = 6;
  }
  if ((stored.preferencesVersion ?? 0) < 7) {
    const occupied = new Set(Object.entries(settings.shortcuts).filter(([action]) => action !== 'improvePrompt').map(([, value]) => shortcutIdentity(value)));
    const legacy = shortcutIdentity('Ctrl+Win+P');
    const desired = shortcutIdentity('Ctrl+Shift+P');
    if (shortcutIdentity(settings.shortcuts.improvePrompt) === legacy && !occupied.has(desired)) settings.shortcuts.improvePrompt = 'Ctrl+Shift+P';
    else if (occupied.has(shortcutIdentity(settings.shortcuts.improvePrompt))) {
      const available = ['Ctrl+Win+P', 'Ctrl+Win+Shift+P', ...Array.from({length:24},(_,i)=>`Ctrl+Win+F${i+1}`)].find(chord=>!occupied.has(shortcutIdentity(chord)));
      if (available) settings.shortcuts.improvePrompt = available;
    }
    settings.preferencesVersion = 7;
  }
  return settings;
}

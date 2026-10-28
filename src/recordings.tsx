// SPDX-License-Identifier: MIT
import { For, Show } from 'solid-js';
import AudioLines from 'lucide-solid/icons/audio-lines';
import Download from 'lucide-solid/icons/download';
import RotateCcw from 'lucide-solid/icons/rotate-ccw';
import Trash2 from 'lucide-solid/icons/trash-2';
import type { SavedAudio } from './types';

export interface RecordingActionsProps {
  clip: SavedAudio;
  busy: boolean;
  available: boolean;
  loading: boolean;
  listen: (id: string) => void;
  download: (id: string) => void;
  retry: (id: string) => void;
  remove?: (id: string) => void;
  allowRemove?: boolean;
}

export function RecordingActions(props: RecordingActionsProps) {
  return <div class="recording-actions">
    <button class="quiet" disabled={!props.available || props.loading} onClick={() => props.listen(props.clip.id)}><AudioLines size={14}/> Listen</button>
    <button class="quiet" aria-label="Download recording" disabled={!props.available || props.loading} onClick={() => props.download(props.clip.id)}><Download size={14}/> Audio</button>
    <button class="secondary" aria-label="Retry transcription" disabled={!props.available || props.busy} onClick={() => props.retry(props.clip.id)}><RotateCcw size={14}/> Retry</button>
    <Show when={props.remove && props.allowRemove !== false}><button class="icon-button" aria-label="Delete saved audio" disabled={props.busy} onClick={() => props.remove?.(props.clip.id)}><Trash2 size={15}/></button></Show>
  </div>;
}

export function RecordingsPanel(props: Omit<RecordingActionsProps, 'clip'> & { clips: SavedAudio[]; compact?: boolean }) {
  const clips = () => props.compact ? props.clips.filter(clip => clip.status !== 'ready').slice(0, 5) : props.clips;
  return <Show when={clips().length}><section class="panel saved-audio">
    <div class="section-title"><h2>{props.compact ? 'Recordings to retry' : 'Saved recordings'} <span>{clips().length}</span></h2><span class="tag">ON THIS DEVICE</span></div>
    <p>Your recording stays available after a retry. Retry uses your current speech and writing settings; review the result here before pasting.</p>
    <For each={clips()}>{clip => <article class="collection-row recording-row"><div>
      <strong><AudioLines size={16}/> {Math.max(1, Math.round(clip.durationMs / 1000))} second recording <span class="tag">{clip.status === 'ready' ? 'Transcribed' : clip.status === 'pending' ? 'Saved before processing' : 'Needs retry'}</span></strong>
      <span>{new Date(clip.createdAt).toLocaleString()} · {clip.inMemory ? 'Memory only · lost when Hush quits' : 'Encrypted on this computer'}{clip.attempts > 1 ? ` · ${clip.attempts} attempts` : ''}</span>
      <Show when={clip.lastError}><span class="recording-error">{clip.lastError}</span></Show>
      <Show when={clip.requiresReview}><span>Audio was interrupted. Check the result for missing words.</span></Show>
    </div><RecordingActions {...props} clip={clip}/></article>}</For>
    <p class="field-note">Encrypted audio is kept for 30 days, up to 1,000 recordings or 512 MB. Private mode and disabled recording storage keep only the latest three in memory. Clear history also clears recordings.</p>
  </section></Show>;
}

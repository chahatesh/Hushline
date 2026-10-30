// SPDX-License-Identifier: MIT
import { createSignal, createEffect, createMemo, Show } from 'solid-js';
import Pencil from 'lucide-solid/icons/pencil';
import ShieldCheck from 'lucide-solid/icons/shield-check';
import { call, desktop } from './bridge';
import type { Settings, PromptImproveMode } from './types';

export function PromptEditor(props: {settings: Settings; busy: boolean; hasPreview: boolean; run: (work:()=>Promise<unknown>,message?:string)=>Promise<boolean>}) {
  const [input,setInput]=createSignal(''),[guidance,setGuidance]=createSignal('');
  const [mode,setMode]=createSignal<PromptImproveMode>(props.settings.promptImproveMode);
  const [sending,setSending]=createSignal(false);
  const savedMode=createMemo(()=>props.settings.promptImproveMode);
  createEffect(()=>setMode(savedMode()));
  const bytes=()=>new TextEncoder().encode(input()).length;
  const guidanceBytes=()=>new TextEncoder().encode(guidance()).length;
  async function generate(event: SubmitEvent) {
    event.preventDefault(); if(props.busy||sending()||!input().trim()||bytes()>32000||guidanceBytes()>2000)return;
    setSending(true);
    try { await props.run(()=>call('improve_prompt_text',{text:input(),mode:mode(),guidance:guidance()})); }
    finally {setSending(false);}
  }
  return <section class="panel prompt-editor"><div class="section-title"><div><h2>{props.hasPreview?'Start another prompt':'Turn a rough request into a clear prompt'}</h2><p>Paste or write a request here. Choose how much to change, then review the result.</p></div><Pencil size={21}/></div><form onSubmit={e=>void generate(e)}>
    <label>Your prompt<textarea aria-label="Your prompt" aria-describedby="prompt-size" rows={8} placeholder="What would you like the AI to do? Include any context or constraints you already have." value={input()} onInput={e=>setInput(e.currentTarget.value)} disabled={props.busy||sending()}/><small id="prompt-size" classList={{'input-error':bytes()>32000}}>{bytes().toLocaleString()} / 32,000 bytes · prompts stay here only for this session</small></label>
    <div class="prompt-options" role="group" aria-label="Edit this prompt"><button type="button" classList={{selected:mode()==='clarify'}} aria-pressed={mode()==='clarify'} onClick={()=>setMode('clarify')} disabled={props.busy}><strong>Clarify</strong><small>Make your request direct and well structured. Keep the same requirements.</small></button><button type="button" classList={{selected:mode()==='expand'}} aria-pressed={mode()==='expand'} onClick={()=>setMode('expand')} disabled={props.busy}><strong>Expand</strong><small>Also suggest useful additions, clearly marked as optional.</small></button></div>
    <label>How should it improve? <span class="optional">optional</span><input value={guidance()} placeholder="e.g. Be concise, group the requirements, keep the technical detail" onInput={e=>setGuidance(e.currentTarget.value)} disabled={props.busy||sending()}/><Show when={guidanceBytes()>2000}><small class="input-error">Editing preferences must be under 2,000 bytes.</small></Show></label>
    <div class="button-row"><button type="submit" class="primary" disabled={!desktop||props.busy||sending()||!input().trim()||bytes()>32000||guidanceBytes()>2000}><Pencil size={15}/>{props.busy||sending()?'Improving…':'Improve this prompt'}</button><button type="button" class="quiet" disabled={props.busy||sending()||!input()} onClick={()=>{setInput('');setGuidance('');}}>Clear draft</button></div>
    <p class="field-note"><ShieldCheck size={14}/> {props.settings.provider==='offline'?'Uses only your configured local Ollama model.':'Sends only this prompt and your editing preference to your Groq Command Mode model.'} No microphone, automatic sending, or prompt history.</p>
  </form><div class="prompt-shortcut-note"><strong>Edit in another app</strong><p>Focus the text box containing your prompt and press {props.settings.shortcuts.improvePrompt.replaceAll('+',' + ')}. Hush reads that field, shows an editable preview, and checks the original again before you choose Apply. If the app does not expose its text safely, paste the prompt here.</p></div></section>;
}

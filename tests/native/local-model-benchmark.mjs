// SPDX-License-Identifier: MIT
// Explicit opt-in benchmark. Own loopback server, workspace-only models, synthetic
// text, no microphone, no cloud inference, no changes to the user's Ollama setup.
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
const executable = process.argv[2] || 'ollama';
const root = path.resolve('.tools/local-text');
fs.mkdirSync(root, { recursive: true });
const host = 'http://127.0.0.1:11439';
try { await fetch(`${host}/api/version`, { signal: AbortSignal.timeout(500) }); throw new Error('Benchmark port is already in use.'); }
catch (e) { if (String(e).includes('already in use')) throw e; }
const server = spawn(executable, ['serve'], { windowsHide: true, stdio: 'ignore', env: {
  ...process.env, OLLAMA_HOST: '127.0.0.1:11439', OLLAMA_MODELS: path.join(root, 'models'),
  OLLAMA_NO_CLOUD: '1', OLLAMA_NUM_PARALLEL: '1', OLLAMA_MAX_LOADED_MODELS: '1',
} });
const model = 'qwen3:0.6b';
const post = async (endpoint, body) => {
  const r = await fetch(`${host}/api/${endpoint}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body), signal: AbortSignal.timeout(900000) });
  if (!r.ok) throw new Error(`Local benchmark ${endpoint}: HTTP ${r.status}`);
  return r.json();
};
function memory() {
  const script = `$ids=[Collections.Generic.HashSet[int]]::new(); [void]$ids.Add(${server.pid}); $all=Get-CimInstance Win32_Process; do {$added=$false; foreach($p in $all){if($ids.Contains([int]$p.ParentProcessId)-and $ids.Add([int]$p.ProcessId)){$added=$true}}}while($added); $sum=0L;foreach($id in $ids){$p=Get-Process -Id $id -ErrorAction SilentlyContinue;if($p){$sum+=$p.WorkingSet64}};[math]::Round($sum/1MB,2)`;
  return Number(execFileSync('powershell.exe', ['-NoProfile', '-Command', script], { windowsHide: true, encoding: 'utf8' }).trim());
}
const report = { model, scope: 'Synthetic English editing fixtures; own CPU service, four threads, 2048 context; not speech recognition or general quality certification', samples: [] };
try {
  let ready = false;
  for (let i=0; i<100; i++) { try { const r=await fetch(`${host}/api/version`); if(r.ok){report.runtime=await r.json();ready=true;break;} } catch {} await new Promise(r=>setTimeout(r,100)); }
  if(!ready) throw new Error('Local benchmark server did not start.');
  report.idleServerWorkingSetMiB = memory();
  console.log('Downloading the small local text model into the ignored workspace benchmark folder…');
  await post('pull', { model, stream: false });
  const metadata = await post('show', { model });
  if(metadata.remote_host || metadata.remote_model || !metadata.model_info?.['general.architecture']) throw new Error('Model is not verified local.');
  const tags = await (await fetch(`${host}/api/tags`)).json();
  report.modelInfo = tags.models.find(m=>m.name===model);
  for(const input of [
    'um I I I want to send the report to Maya on Thursday at 3 pm and and make sure the build number is v2.4.1',
    'Meeting notes. First, review the draft. Second, test the installer. Third, email Maya at maya@example.com.',
    'We reviewed the release today. The build passed all checks. The installer still needs testing on another machine. We should preserve the current configuration during an upgrade. The API endpoint is https://example.com/v2 and the variable is useState. On another note, we need to prepare the documentation. Maya will review it on Thursday. Please keep the deadline at 3 pm and do not change build number v2.4.1.',
  ]) {
    const started=performance.now();
    const output=await post('chat',{model,stream:false,think:false,keep_alive:'1m',options:{temperature:0,num_ctx:2048,num_predict:1024,num_thread:4,num_gpu:0},messages:[
      {role:'system',content:'Edit the transcript into clear, natural, professional English. Remove stutters and accidental repeated words. Use paragraphs for longer prose and • bullets for a clear list. Preserve every fact, name, number, URL, code identifier and substantive statement. Return only the edited text, no commentary. Do not add facts.'},
      {role:'user',content:input},
    ]});
    report.samples.push({input,output:output.message?.content,wallMs:Math.round(performance.now()-started),loadMs:Math.round(output.load_duration/1e6),evaluationMs:Math.round(output.eval_duration/1e6),generatedTokens:output.eval_count,doneReason:output.done_reason,observedResidentWorkingSetMiB:memory()});
    console.log(`Local edit ${report.samples.length}: ${report.samples.at(-1).wallMs} ms, ${report.samples.at(-1).observedResidentWorkingSetMiB} MiB service working set`);
  }
  await post('generate',{model,keep_alive:0});
} finally {
  if(server.exitCode===null) { try { execFileSync('taskkill.exe',['/pid',String(server.pid),'/t','/f'],{windowsHide:true,stdio:'ignore'}); } catch {} }
  fs.mkdirSync('artifacts',{recursive:true}); fs.writeFileSync('artifacts/local-model-benchmark.json',JSON.stringify(report,null,2)+'\n');
}

// SPDX-License-Identifier: MIT
// Run manually against a built executable. Remote debugging is test-only, local,
// supplied by this process environment, and never enabled in the shipped config.
import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const executable=path.resolve(process.argv[2] || 'src-tauri/target/release/hushline.exe');
if(!fs.existsSync(executable)) throw new Error('Build the native executable first.');
const port=9227;
const started=performance.now();
const child=spawn(executable,[],{windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`}});
const report={configuration:'release executable, test-only CDP, actual native IPC',desktopBootObservedMs:null,checks:{},error:null};
let browser,main,addedWord=false,savedDummy=false;
const word=`hushline-native-smoke-${Date.now()}`;
const native=(page,command,args={})=>page.evaluate(({command,args})=>window.__TAURI_INTERNALS__.invoke(command,args),{command,args});
async function waitFor(fn,timeout=30000){const limit=performance.now()+timeout;while(performance.now()<limit){try{const result=await fn();if(result)return result;}catch{/* The runtime may still be initializing. */}await new Promise(r=>setTimeout(r,100));}throw new Error('Timed out waiting for the actual desktop application.');}
try {
  await waitFor(async()=>{const r=await fetch(`http://127.0.0.1:${port}/json/version`);return r.ok;});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  main=await waitFor(async()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()!=='about:blank'&&!p.url().includes('overlay=1')));
  await main.getByRole('heading',{name:'Let your thoughts flow.'}).waitFor({timeout:30000});
  report.desktopBootObservedMs=Math.round(performance.now()-started);
  assert.equal(await main.getByText('Browser preview',{exact:true}).count(),0);
  const before=await native(main,'snapshot');
  report.checks.nativeSnapshot=true;
  report.checks.noSecretInSettings=!('apiKey' in before.settings);
  // A developer may already have real dictation in their profile. Never publish it.
  await main.screenshot({path:'docs/screenshots/windows-home.png',fullPage:true,animations:'disabled',timeout:20000,maskColor:'#e4e9f0',mask:[main.locator('.transcript-row'),main.locator('.recovery'),main.locator('.stat-strip')]});
  await main.getByRole('button',{name:'Dictionary',exact:true}).click();
  await main.getByLabel('Word or phrase').fill(word);await main.getByLabel('Replace with').fill('NativeSmoke');await main.getByRole('button',{name:'Add word',exact:true}).click();
  await main.getByRole('button',{name:`Edit ${word}`}).waitFor();addedWord=true;
  const saved=await native(main,'snapshot');assert(saved.dictionary.some(e=>e.source===word));
  await main.reload();await main.getByRole('button',{name:'Dictionary',exact:true}).click();await main.getByRole('button',{name:`Edit ${word}`}).waitFor();
  report.checks.nativeDictionaryPersisted=true;
  await main.getByRole('button',{name:`Delete ${word}`}).click();await waitFor(async()=>!(await native(main,'snapshot')).dictionary.some(e=>e.source===word));addedWord=false;
  await main.getByRole('button',{name:'Settings',exact:true}).click();
  if(!before.view.hasKey){
    await main.locator('input[type=password]').fill('hushline-ui-smoke-not-a-real-groq-key');await main.getByRole('button',{name:'Save key',exact:true}).click();
    await waitFor(async()=>(await native(main,'snapshot')).view.hasKey);savedDummy=true;
    await main.getByText('Ready to dictate',{exact:true}).waitFor();
    await main.getByRole('button',{name:'Remove key',exact:true}).click();await waitFor(async()=>!(await native(main,'snapshot')).view.hasKey);savedDummy=false;
    report.checks.nativeCredentialUIRoundtrip=true;
  }else report.checks.nativeCredentialUIRoundtrip='skipped: existing key preserved';
  const baselineHistory=(await native(main,'snapshot')).history.length;
  await main.getByRole('button',{name:'Test',exact:true}).click();
  await waitFor(async()=>(await native(main,'snapshot')).view.phase==='listening');
  await native(main,'action',{name:'cancel'});await waitFor(async()=>(await native(main,'snapshot')).view.phase==='idle');
  assert.equal((await native(main,'snapshot')).history.length,baselineHistory);
  report.checks.nativeMicrophoneTestCancel=true;
  const backup=await native(main,'export_configuration');assert(!('apiKey' in backup)&&!('history' in backup)&&!('scratchpad' in backup));report.checks.nativeBackupExcludesSensitiveData=true;
  report.checks.liveGroqToNotepad='not tested: no live provider request; native insertion harness could not acquire foreground';
  const closed=main.waitForEvent('close',{timeout:10000});
  execFileSync('powershell.exe',['-NoProfile','-File','scripts/close-owned-dashboard.ps1','-ApplicationProcessId',String(child.pid),'-Executable',executable],{windowsHide:true});
  await closed;main=undefined;
  await new Promise(r=>setTimeout(r,500));assert.equal(child.exitCode,null,'Closing the dashboard exited the tray engine');
  await browser.close().catch(()=>{});browser=undefined;
  spawn(executable,[],{windowsHide:true,stdio:'ignore',env:process.env});
  await waitFor(async()=>{const r=await fetch(`http://127.0.0.1:${port}/json/version`);return r.ok;});
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  main=await waitFor(async()=>browser.contexts().flatMap(c=>c.pages()).find(p=>p.url()!=='about:blank'));
  await main.getByRole('heading',{name:'Let your thoughts flow.'}).waitFor({timeout:30000});
  report.checks.nativeDashboardCloseTrayAndReopen=true;
}catch(error){report.error=String(error);process.exitCode=1;}
finally{
  if(main){
    try{if(addedWord){const s=await native(main,'snapshot');await native(main,'save_dictionary',{entries:s.dictionary.filter(e=>e.source!==word)});}if(savedDummy)await native(main,'delete_api_key');await native(main,'action',{name:'cancel'});}catch{/* Report retains any failure; never read a real saved key. */}
  }
  if(browser)await browser.close();
  child.kill();
  fs.mkdirSync('artifacts',{recursive:true});fs.writeFileSync('artifacts/desktop-smoke.json',JSON.stringify(report,null,2));
  console.log(JSON.stringify(report,null,2));
}

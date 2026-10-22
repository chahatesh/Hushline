// SPDX-License-Identifier: MIT
// Actual native clipboard + input/focus hooks against fresh, local browser fields.
// No network, microphone, API key, existing browser profile, or user document.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const tests = fs.readdirSync('src-tauri/target/debug/deps').filter(n => /^hushline_lib-.*\.exe$/.test(n)).map(n => path.resolve('src-tauri/target/debug/deps', n)).sort((a,b)=>fs.statSync(b).mtimeMs-fs.statSync(a).mtimeMs);
assert(tests.length, 'Build Rust library tests first');
const browser = await chromium.launch({channel:'chrome',headless:false,args:['--force-renderer-accessibility']});
const report={kind:'actual native clipboard insertion; no speech provider',checks:{},error:null};
try {
  const page=await browser.newPage();
  for(const kind of ['textarea','contenteditable']) {
    const title=`Hush owned paste test ${Date.now()} ${kind}`;
    await page.setContent(`<title>${title}</title><label>Owned paste field ${kind}</label>${kind==='textarea'?'<textarea aria-label="Owned paste field" style="width:600px;height:300px"></textarea>':'<div role="textbox" aria-label="Owned paste field" contenteditable="true" style="width:600px;height:300px;border:1px solid"></div>'}`);
    const field=page.getByRole('textbox',{name:'Owned paste field',exact:true});
    await field.fill('Before: ');await field.click();await field.press('Control+End');await page.bringToFront();
    await new Promise((resolve,reject)=>{
      const child=spawn(tests[0],['native_browser_clipboard','--ignored','--nocapture','--test-threads=1'],{windowsHide:true,env:{...process.env,HUSH_BROWSER_TEST_TITLE:title}});
      let output='';child.stdout.on('data',x=>output+=x);child.stderr.on('data',x=>output+=x);
      const timeout=setTimeout(()=>{child.kill();reject(new Error('Native paste test timed out'));},20000);
      child.on('error',reject);child.on('exit',code=>{clearTimeout(timeout);console.log(output);code===0?resolve():reject(new Error(`${kind}: native insertion failed`));});
    });
    const expected='Before: Hush clipboard: first sentence.\n\n• First item\n• Second item';
    await assert.doesNotReject(async()=>{
      if(kind==='textarea')await field.evaluate((el,expected)=>{if(el.value!==expected)throw new Error('Textarea did not receive the complete transcript');},expected);
      else await field.evaluate((el,expected)=>{if(el.innerText.replace(/\r\n/g,'\n')!==expected)throw new Error('Editable field did not receive the complete transcript');},expected);
    });
    report.checks[kind]=true;
  }
}catch(e){report.error=String(e);process.exitCode=1;}
finally{await browser.close();fs.mkdirSync('artifacts',{recursive:true});fs.writeFileSync('artifacts/browser-paste.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}

// SPDX-License-Identifier: MIT
// Real Windows delivery into disposable fields; no microphone, cloud or user profile.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const tests=fs.readdirSync('src-tauri/target/debug/deps').filter(n=>/^hushline_lib-.*\.exe$/.test(n)).map(n=>path.resolve('src-tauri/target/debug/deps',n)).sort((a,b)=>fs.statSync(b).mtimeMs-fs.statSync(a).mtimeMs);
assert(tests.length,'Build Rust library tests first');
const browser=await chromium.launch({channel:'chrome',headless:false,args:['--force-renderer-accessibility']});
const report={kind:'actual native clipboard insertion with live hooks; no speech provider',contenteditableWhiteSpace:'pre-wrap',checks:{},error:null};
try {
  const page=await browser.newPage();
  for(const kind of ['textarea','contenteditable']) for(const mode of ['stable','reflow','changed_field']) {
    const title=`Hush owned paste test ${Date.now()} ${kind} ${mode}`;
    const control=label=>kind==='textarea'?`<textarea aria-label="${label}" style="width:600px;height:200px"></textarea>`:`<div role="textbox" aria-label="${label}" contenteditable="true" style="width:600px;height:200px;border:1px solid;white-space:pre-wrap"></div>`;
    await page.setContent(`<title>${title}</title>${control('Owned paste field')}${control('Other owned field')}`);
    const field=page.getByRole('textbox',{name:'Owned paste field',exact:true});
    const other=page.getByRole('textbox',{name:'Other owned field',exact:true});
    await other.fill('Other field');await field.fill('Before: ');
    await field.click();await field.press('Control+End');await page.bringToFront();
    await new Promise((resolve,reject)=>{
      const child=spawn(tests[0],['native_browser_clipboard','--ignored','--nocapture','--test-threads=1'],{windowsHide:true,env:{...process.env,HUSH_BROWSER_TEST_TITLE:title,HUSH_BROWSER_MUTATE:mode==='stable'?'0':'1',HUSH_BROWSER_CHANGE_FIELD:mode==='changed_field'?'1':'0'}});
      let output='',mutated=false;
      const timeout=setTimeout(()=>{child.kill();reject(new Error('Native paste test timed out'));},20000);
      async function mutate(){
        if(mode==='changed_field')await other.evaluate(el=>el.focus());
        else {
          const moved=await field.evaluate(el=>{const before=el.getBoundingClientRect().y;el.style.display='block';el.style.marginTop='80px';el.style.height='240px';return el.getBoundingClientRect().y!==before;});
          assert(moved,'The reflow fixture must actually move the field');
        }
        child.stdin.end('continue\n');
      }
      child.stdout.on('data',chunk=>{output+=chunk;if(mode!=='stable'&&!mutated&&output.includes('HUSH_BROWSER_TARGET_READY')){mutated=true;mutate().catch(error=>{child.kill();reject(error);});}});
      child.stderr.on('data',chunk=>output+=chunk);
      child.on('error',error=>{clearTimeout(timeout);reject(error);});
      child.on('exit',code=>{clearTimeout(timeout);console.log(output);code===0&&(mode==='stable'||mutated)?resolve():reject(new Error(`${kind}/${mode}: native insertion failed`));});
    });
    const expected=mode==='changed_field'?'Before: ':'Before: Hush clipboard: first sentence.\n\n• First item\n• Second item';
    const actual=kind==='textarea'?await field.inputValue():await field.innerText();
    assert.equal(actual.replace(/\r\n/g,'\n'),expected,`${kind}/${mode}: complete text must arrive exactly once`);
    assert.equal(kind==='textarea'?await other.inputValue():await other.innerText(),'Other field','Never insert into a different field');
    report.checks[`${kind}_${mode}`]=true;
  }
}catch(error){report.error=String(error);process.exitCode=1;}
finally{await browser.close();fs.mkdirSync('artifacts',{recursive:true});fs.writeFileSync('artifacts/browser-paste.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));}

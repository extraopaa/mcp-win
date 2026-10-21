// Synthetic files only. The PowerShell runner restores the user's preferences.
const {chromium}=require('../tools/dev/node_modules/playwright');
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
let browser;
(async()=>{
 for(let i=0;i<100;i++){try{browser=await chromium.connectOverCDP('http://127.0.0.1:9223');break}catch{await new Promise(r=>setTimeout(r,300))}}
 assert(browser);let page;for(let i=0;i<100;i++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));if(page)break;await new Promise(r=>setTimeout(r,200));}assert(page);
 const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const invoke=(name,args={})=>page.evaluate(([n,a])=>window.__TAURI__.core.invoke(n,a),[name,args]);
 await page.waitForFunction(()=>window.MCPWinOffice&&document.getElementById('token').value.length>20);
 assert.equal(await page.locator('.brand-version').textContent(),'v'+require('../tauri.conf.json').version);
 assert.equal((await invoke('update_status')).current_version,require('../tauri.conf.json').version);
 const snapshot=await invoke('snapshot'),folder=snapshot.folders[0];assert(folder&&!folder.writable);
 const source=path.join(folder.path,'office.docx'),original=fs.readFileSync(source);
 const run=(fn,args)=>page.evaluate(fn,args);
 await run(async folder=>{await MCPWinOffice.openShared(folder,'office.docx')},folder.id);
 await run(async()=>{await MCPWinOffice.command('text.insert',{text:'Native save validation.'})});
 await page.waitForFunction(()=>MCPWinOffice.active.dirty);
 await page.waitForTimeout(900);await assert.rejects(()=>invoke('install_update'));
 assert.equal(await run(()=>MCPWinOffice.save()),false);assert.deepEqual(fs.readFileSync(source),original);
 await invoke('set_folder_writable',{id:folder.id,writable:true});
 assert.equal(await run(()=>MCPWinOffice.save()),true);assert.notDeepEqual(fs.readFileSync(source),original);
 const history=await invoke('list_history',{folder:folder.id});assert(JSON.stringify(history).includes('office.docx'));
 fs.appendFileSync(source,'\nexternal-change');const external=fs.readFileSync(source);
 await run(async()=>{await MCPWinOffice.command('text.insert',{text:'Concurrent edit.'})});
 await page.waitForFunction(()=>MCPWinOffice.active.dirty);
 assert.equal(await run(()=>MCPWinOffice.save()),false);assert.deepEqual(fs.readFileSync(source),external);
 await run(()=>{MCPWinOffice.close()});await page.locator('#office-unsaved').waitFor({state:'visible'});
 await page.locator('#office-cancel-close').click();assert.equal(await page.locator('#office-frames iframe').count(),1);
 await run(()=>{MCPWinOffice.close()});await page.locator('#office-discard').click();
 await page.waitForFunction(()=>!MCPWinOffice.active);fs.writeFileSync(source,original);
 if(await page.locator('#toast').isVisible())await page.locator('#dismiss-toast').click();
 for(const kind of ['word','grid','deck','pdf','design']){
  await page.locator(`[data-office-editor="${kind}"]`).click();
  await page.waitForFunction(k=>MCPWinOffice.active?.kind===k&&MCPWinOffice.active?.ready&&!MCPWinOffice.active?.opening,kind,{timeout:180000});
  if(kind==='word'){await run(()=>MCPWinOffice.command('text.insert',{text:'Orbite Studio\nVos documents, dans MCP Win.\n\nTexte, tableurs, présentations, PDF et composition : cinq éditeurs locaux réunis dans votre espace de travail.'}));await page.waitForTimeout(350);}
  assert.equal(await page.locator('#office-frames iframe:visible').count(),1);
  assert.equal(await page.locator(`[data-office-editor="${kind}"].active`).count(),1);
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  await page.screenshot({path:`target/qa-office/native-${kind}.png`});console.log('PASS native',kind);
 }
 await run(async folder=>{await MCPWinOffice.openShared(folder,'office.pdf')},folder.id);
 await page.locator('#office-ocr').click();await page.locator('#office-run-ocr').click();
 await page.waitForFunction(()=>!document.getElementById('office-run-ocr').disabled,null,{timeout:120000});
 assert((await page.locator('#office-ocr-text').inputValue()).length>30);
 await page.locator('#office-close-ocr').click();await run(()=>MCPWinOffice.close());
 if(!process.env.OFFICE_SKIP_LAYOUT){
 await page.setViewportSize({width:900,height:680});
 await page.waitForTimeout(400);
 const bounds=await page.locator('#office-frames').boundingBox();assert(bounds&&bounds.y+bounds.height<=680);
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 await page.screenshot({path:'target/qa-office/native-small.png'});
 }
 assert.deepEqual(errors,[]);
 const report={readonly:true,save:true,history:true,conflict:true,unsavedCancel:true,sidebarEditors:5,ocr:true,errors};
 fs.writeFileSync('target/qa-office/native-report.json',JSON.stringify(report,null,2));console.log(report);
})().catch(e=>{console.error(e);process.exitCode=1}).finally(async()=>{if(browser)await browser.close()});

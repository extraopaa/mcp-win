// Synthetic fixtures only, invoked by run-v1-native-check.ps1.
const {chromium}=require('../tools/dev/node_modules/playwright');
const assert=require('node:assert/strict');
const fs=require('node:fs');
let browser;
(async()=>{
 for(let i=0;i<80;i++){try{browser=await chromium.connectOverCDP('http://127.0.0.1:9223');break}catch{await new Promise(r=>setTimeout(r,250))}}
 assert(browser);let page;
 for(let i=0;i<80;i++){page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));if(page)break;await new Promise(r=>setTimeout(r,250))}
 assert(page);const errors=[];page.on('pageerror',e=>errors.push(e.message));
 const invoke=(name,args={})=>page.evaluate(([n,a])=>window.__TAURI__.core.invoke(n,a),[name,args]);
 await page.waitForFunction(()=>document.getElementById('token').value.length>20);
 for(const tab of ['overview','shared-folders','connection','activity','tools','studio']){
   await page.locator(`[data-tab="${tab}"]`).click();assert.equal(await page.locator('.page:visible').count(),1);
   assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 }
 const initial=await invoke('snapshot');assert(!initial.running);
 await page.locator('[data-tab=shared-folders]').click();await page.locator('.write-permission').first().click();
 await page.waitForFunction(async()=> (await window.__TAURI__.core.invoke('snapshot')).folders[0].writable);
 await page.locator('[data-tab=overview]').click();await page.locator('#toggle-server').click();
 await page.waitForFunction(async()=> (await window.__TAURI__.core.invoke('snapshot')).ready,null,{timeout:20000});
 const state=await invoke('snapshot');assert.equal(state.connection.mode,'local');
 let rpcId=0;
 async function rpc(method,params){const r=await fetch(state.address,{method:'POST',headers:{'Content-Type':'application/json',Accept:'application/json, text/event-stream','MCP-Protocol-Version':'2025-03-26',Authorization:`Bearer ${state.connection.token}`},body:JSON.stringify({jsonrpc:'2.0',id:++rpcId,method,params}),signal:AbortSignal.timeout(60000)});assert.equal(r.status,200);const p=await r.json();assert(!p.error,JSON.stringify(p));return p.result}
 async function tool(name,args={}){const r=await rpc('tools/call',{name,arguments:{folder:'ui-test',...args}});assert(!r.isError,JSON.stringify(r));return {value:JSON.parse(r.content[0].text),content:r.content}}
 assert.equal((await rpc('tools/list',{})).tools.length,42);
 assert.equal((await fetch(state.address,{method:'POST',body:'{}'})).status,401);
 const doc=(await tool('create_document',{path:'native-v1.pdf',title:'Rapport professionnel',blocks:[{id:'summary',kind:'heading',level:1,text:'Synthèse'},{id:'body',kind:'paragraph',text:'La v1 produit des documents soignés par défaut.'},{id:'table',kind:'table',columns:['Indicateur','Valeur'],rows:[['Dossiers','125']]}]})).value;
 assert.equal(doc.design.mode,'professional');assert(doc.quality.passed);
 const review=await tool('review_document',{path:doc.path,pages:[1]});assert(review.content.some(c=>c.type==='image'));
 await tool('index_folder');const search=(await tool('search_index',{query:'professionnel'})).value;
 assert(JSON.stringify(search).includes('native-v1.pdf'));
 const structure=(await tool('inspect_document_elements',{path:doc.path})).value;assert(structure.elements.some(e=>e.id==='body'));
 const proposal=(await tool('edit_document_elements',{path:doc.path,output_path:'corrected.pdf',expected_sha256:doc.sha256,edits:[{id:'body',text:'Une correction contrôlée et réversible.'}]})).value;
 await assert.rejects(()=>invoke('revision_action',{id:proposal.revision_id,action:'apply'}));
 const job=(await tool('start_workflow',{name:'Livrables v1',steps:[{id:'note',tool:'write_text_file',args:{path:'workflow.txt',content:'Durable'}},{id:'zip',tool:'create_archive',args:{paths:[{'$ref':'note.path'}],output_path:'workflow.zip'}}]})).value;
 let status;for(let i=0;i<100;i++){status=(await tool('workflow_status',{id:job.id})).value;if(status.status!=='running')break;await new Promise(r=>setTimeout(r,100))}assert.equal(status.status,'completed');
 await page.locator('#toggle-server').click();await page.waitForFunction(async()=> !(await window.__TAURI__.core.invoke('snapshot')).running);
 const previews=await invoke('preview_revision',{id:proposal.revision_id});assert(previews.images.length>=2);
 await invoke('revision_action',{id:proposal.revision_id,action:'apply'});
 await invoke('revision_action',{id:proposal.revision_id,action:'undo'});
 await invoke('workflow_action',{id:job.id,action:'undo'});
 const workspace=await invoke('v1_workspace');assert(workspace.reviews.revisions.some(r=>r.id===proposal.revision_id&&r.status==='undone'));
 await page.locator('[data-tab=studio]').click();await page.screenshot({path:'target/qa-v1/native-studio.png'});
 assert.deepEqual(errors,[]);fs.writeFileSync('target/qa-v1/native-validation.json',JSON.stringify({tools:42,default_design:true,quality:true,index:true,revisions:true,workflow:true,auth:true},null,2));
 console.log('Tauri v1 : 42 outils, PDF professionnel, aperçu, index, révision validée/annulée, workflow et annulation groupée, authentification et six vues validés.');
})().catch(e=>{console.error(e);process.exitCode=1}).finally(async()=>{await browser?.close()});

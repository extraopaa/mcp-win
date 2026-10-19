const { chromium } = require('../tools/dev/node_modules/playwright');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
let connectedBrowser;
(async () => {
  let browser;
  for(let attempt=0;attempt<20;attempt++) {
    try {browser=await chromium.connectOverCDP('http://127.0.0.1:9223');break;} catch {await new Promise(r=>setTimeout(r,500));}
  }
  assert(browser,'Tauri did not open its WebView');
  connectedBrowser=browser;
  let page;
  for(let attempt=0;attempt<40;attempt++) {
    page=browser.contexts().flatMap(c=>c.pages()).find(p=>/tauri|localhost/.test(p.url()));
    if(page)break;
    await new Promise(r=>setTimeout(r,250));
  }
  assert(page,'Native Tauri page did not load');
  await page.waitForFunction(()=>document.getElementById('token').value.length>20);
  let result;
  try {
    const before=await page.evaluate(()=>window.__TAURI__.core.invoke('snapshot'));
    assert.equal(before.folders[0].writable,false);
    await page.locator('.write-permission').first().click();
    for(let attempt=0;attempt<20;attempt++){const s=await page.evaluate(()=>window.__TAURI__.core.invoke('snapshot'));if(s.folders[0].writable)break;await new Promise(r=>setTimeout(r,100));}
    await page.locator('#toggle-server').click();
    let state;
    for(let attempt=0;attempt<80;attempt++) {
      state=await page.evaluate(()=>window.__TAURI__.core.invoke('snapshot'));
      if(state.ready)break;
      if(!state.running && state.logs.length>1)break;
      await new Promise(r=>setTimeout(r,500));
    }
    assert(state.ready, state.logs.join('\n')); assert.equal(state.connection.require_auth,false);
    await page.evaluate(()=>{navigator.clipboard.writeText=async text=>{window.testCopiedConfig=text;};});
    await page.locator('[data-tab=connection]').click();
    await page.locator('#copy-config').click();
    const copied=JSON.parse(await page.evaluate(()=>window.testCopiedConfig));
    assert.equal(copied.mcpServers['mcp-win'].headers.Authorization,undefined);
    await page.locator('[data-tab=overview]').click();
    const response=await fetch(state.address,{method:'POST',headers:{'Content-Type':'application/json',Accept:'application/json, text/event-stream','MCP-Protocol-Version':'2025-03-26','bypass-tunnel-reminder':'true'},body:JSON.stringify({jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'read_file',arguments:{folder:'ui-test',path:'note.txt'}}}),signal:AbortSignal.timeout(30000)});
    const text=await response.text();
    assert.equal(response.status,200);
    assert(text.includes('NATIVE-TAURI-PUBLIC-OK'),text);
    async function tool(name,args){
      const response=await fetch(state.address,{method:'POST',headers:{'Content-Type':'application/json',Accept:'application/json, text/event-stream','MCP-Protocol-Version':'2025-03-26','bypass-tunnel-reminder':'true'},body:JSON.stringify({jsonrpc:'2.0',id:2,method:'tools/call',params:{name,arguments:{folder:'ui-test',...args}}}),signal:AbortSignal.timeout(30000)});
      const payload=await response.json();assert(!payload.error,JSON.stringify(payload));assert(!payload.result.isError,JSON.stringify(payload.result));return payload.result.content[0].type==='image'?payload.result.content[0]:JSON.parse(payload.result.content[0].text);
    }
    for(const extension of ['pdf','docx','pptx']){
      await tool('create_document',{path:`native.${extension}`,title:'Validation MCP Win',sections:[{heading:'Résultats',text:'Document généré via le tunnel public.',bullets:['Création en Rust','Sans authentification']}]});
      const bytes=await tool('read_file_bytes',{path:`native.${extension}`,max_bytes:64});
      assert(Buffer.from(bytes.data,'base64').subarray(0,extension==='pdf'?5:2).equals(Buffer.from(extension==='pdf'?'%PDF-':'PK')));
    }
    await tool('write_spreadsheet',{output_path:'native.xlsx',cells:[{sheet:'Sheet1',cell:'A1',kind:'text',value:'Création Excel'},{sheet:'Sheet1',cell:'B2',kind:'number',value:'42'}]});
    const workbook=await tool('read_file',{path:'native.xlsx'});assert(workbook.text.includes('42'));
    await tool('create_designed_pdf',{output_path:'designed.pdf',source:fs.readFileSync(path.join(__dirname,'../docs/examples/designed.typ'),'utf8')});
    const preview=await tool('render_pdf_page',{path:'designed.pdf',page:1,width:1200});
    assert.equal(preview.mimeType,'image/png');assert(Buffer.from(preview.data,'base64').length>10000);
    await tool('create_binary_file',{path:'scan.png',data_base64:preview.data});
    const recognized=await tool('ocr_file',{path:'scan.png'});assert(recognized.text.includes('MCP'));assert(recognized.lines.length>3);
    await tool('write_office_package',{output_path:'styled.docx',parts:[{name:'word/document.xml',text:fs.readFileSync(path.join(__dirname,'../docs/examples/document.xml'),'utf8')}]});
    const styled=await tool('inspect_office_package',{path:'styled.docx',part:'word/document.xml'});assert(styled.text.includes('w:color'));
    const note=await tool('read_file',{path:'note.txt'});
    await tool('write_text_file',{path:'note.txt',content:'NATIVE-TAURI-PUBLIC-OK — modifié',expected_sha256:note.sha256});
    await page.screenshot({path:path.resolve(__dirname,'../preview-online.png')});
    result='Native Tauri → public HTTPS MCP: designed PDF, PNG preview, image OCR, styled DOCX, Office generation and text editing passed.';
  } finally {
    await page.evaluate(()=>window.__TAURI__.core.invoke('stop_mcp'));
    await browser.close();
  }
  console.log(result);
})().catch(async error=>{await connectedBrowser?.close();console.error(error);process.exitCode=1;});

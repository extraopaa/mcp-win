const { chromium } = require('../tools/dev/node_modules/playwright');
const assert = require('node:assert/strict');
const path = require('node:path');
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
    await page.screenshot({path:path.resolve(__dirname,'../preview-online.png')});
    result='Native Tauri start → Rust tunnel → public HTTPS MCP → file read: passed.';
  } finally {
    await page.evaluate(()=>window.__TAURI__.core.invoke('stop_mcp'));
    await browser.close();
  }
  console.log(result);
})().catch(async error=>{await connectedBrowser?.close();console.error(error);process.exitCode=1;});

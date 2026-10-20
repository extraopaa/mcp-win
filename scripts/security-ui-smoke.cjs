const { chromium } = require('../tools/dev/node_modules/playwright');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
(async () => {
 const browser = await chromium.launch({channel:'msedge',headless:true});
 try {
 const page = await browser.newPage({viewport:{width:1180,height:820}});
 const errors=[]; page.on('pageerror', e=>errors.push(e.message));
 await page.addInitScript(() => {
  const connection={mode:'cloudflare',port:8787,require_auth:true,token:'0123456789abcdef0123456789abcdef',relay:'https://localtunnel.me',frp_server:'',frp_port:7000,frp_token:'',public_url:'',frp_ca_pem:'',frp_tls_name:''};
  window.testRunning=false; Object.defineProperty(navigator,'clipboard',{value:{writeText:async text=>window.copied=text}});
  window.__TAURI__={core:{invoke: async (name,args)=>{
   if(name==='save_settings') { window.saved=args.connection; return; }
   if(name==='snapshot') return {connection,folders:[{id:'test',name:'Documents de démonstration',path:'C:/Documents/Exemple',writable:true}],running:window.testRunning,ready:window.testRunning,address:null,logs:[]};
   if(name==='list_history') return {versions:[{id:'0123456789abcdef0123456789abcdef',path:'rapport.txt',bytes:42,timestamp:1792440000}]};
   if(name==='restore_version') { window.restored=args; return {path:'rapport.txt'}; }

  }}};
 });
 await page.goto('file:///'+path.resolve(__dirname,'../ui/index.html').replaceAll('\\','/'));
 await page.waitForFunction(()=>document.getElementById('token').value.length===32);
 await page.locator('#go-connection').click();
 assert.equal(await page.locator('[value=localtunnel]').count(),0);
 assert(await page.locator('[value=cloudflare]').isChecked());
 assert(await page.locator('#require-auth').isDisabled());
 assert(await page.locator('#require-auth').isChecked());
 await page.locator('[name=mode][value=frp]').check();
 await page.locator('#frp-ca').fill('-----BEGIN CERTIFICATE-----\nTEST-ONLY');
 await page.locator('#frp-tls-name').fill('relay.example');
 await page.locator('#save-settings').click();
 await page.waitForFunction(()=>window.saved?.frp_tls_name==='relay.example');
 assert.equal(await page.evaluate(()=>window.saved.require_auth), true);
 assert.equal(await page.evaluate(()=>window.saved.frp_ca_pem.includes('TEST-ONLY')),true);
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 assert.deepEqual(errors,[]);
 fs.mkdirSync(path.resolve(__dirname,'../target/security-v04'),{recursive:true});
 await page.screenshot({path:path.resolve(__dirname,'../target/security-v04/connection.png'),fullPage:true});
 await page.locator('#copy-config').click();
 assert.deepEqual(JSON.parse(await page.evaluate(()=>window.copied)),{Authorization:'Bearer 0123456789abcdef0123456789abcdef'});
 await page.locator('#copy-bearer').click();assert.equal(await page.evaluate(()=>window.copied),'Bearer 0123456789abcdef0123456789abcdef');
 await page.locator('[data-tab=shared-folders]').click();
 await page.locator('.history-button').click();
 await page.waitForSelector('.history-row');
 await page.locator('.history-row button').click();
 await page.waitForFunction(()=>window.restored?.version==='0123456789abcdef0123456789abcdef');
 assert.equal(await page.evaluate(()=>window.restored.folder),'test');
 await page.screenshot({path:path.resolve(__dirname,'../target/security-v04/history.png'),fullPage:true});
 await page.locator('#close-history').click();
 await page.evaluate(async()=>{window.testRunning=true;await refresh();});
 await page.locator('.history-button').click();await page.waitForSelector('.history-row');
 assert(await page.locator('.history-row button').isDisabled());
 await page.locator('#close-history').click();
 await page.setViewportSize({width:900,height:820});
 assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 assert.deepEqual(errors,[]);
 console.log('Security/v0.4 UI: auth, headers/Bearer clipboard, CA fields, history restore/disabled while running, no JS errors or overflow.');
 } finally { await browser.close(); }
})().catch(e=>{console.error(e);process.exitCode=1;});

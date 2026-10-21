const {chromium}=require('../tools/dev/node_modules/playwright');
const http=require('node:http'),fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const root=path.resolve(__dirname,'../ui/office'),out=path.resolve(__dirname,'../target/qa-office');
const server=http.createServer((req,res)=>{const pathname=new URL(req.url,'http://localhost').pathname;
 if(pathname==='/favicon.ico'){res.writeHead(204).end();return;}
 if(pathname==='/'){res.writeHead(200,{'Content-Type':'text/html'}).end('<html><body style="margin:0"><iframe id="editor" style="width:100vw;height:100vh;border:0"></iframe></body></html>');return;}
 const file=path.resolve(root,'.'+pathname);if(!file.startsWith(root+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404).end();return;}
 const ext=path.extname(file),mime={'.wasm':'application/wasm','.js':'text/javascript','.html':'text/html','.css':'text/css'}[ext]||'application/octet-stream';
 res.writeHead(200,{'Content-Type':mime,'Content-Security-Policy':"default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data: blob:; connect-src 'self'"});fs.createReadStream(file).pipe(res);
});
(async()=>{require('./office-fixtures.cjs')(out);await new Promise(r=>server.listen(0,'127.0.0.1',r));const browser=await chromium.launch({channel:'msedge',headless:true,args:['--enable-unsafe-swiftshader']});
 try{const page=await browser.newPage({viewport:{width:1280,height:900}}),errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='error')console.log('BROWSER:',m.text())});
 await page.goto('http://127.0.0.1:'+server.address().port);await page.evaluate(()=>{window.messages=[];window.addEventListener('message',e=>{let m=e.data;try{if(typeof m==='string')m=JSON.parse(m)}catch{return}if(m.type==='save')m.bytes=Array.from(new Uint8Array(m.bytes));window.messages.push(m)});window.rpc=async(action,args={})=>{const id=Math.random();document.getElementById('editor').contentWindow.postMessage({channel:'mcp-win-office',id,action,...args},location.origin);for(let i=0;i<600;i++){const m=window.messages.find(x=>x.type==='reply'&&x.id===id);if(m){if(m.error)throw new Error(m.error);return m.result;}await new Promise(r=>setTimeout(r,100));}throw new Error('RPC timeout '+action)}});
 for(const kind of (process.env.OFFICE_KIND?[process.env.OFFICE_KIND]:['word','grid','deck','design','pdf'])){
  console.log('START',kind);await page.evaluate(()=>window.messages=[]);await page.locator('#editor').evaluate((e,k)=>e.src='/editor.html?kind='+k,kind);
  await page.waitForFunction(()=>window.messages.some(m=>m.type==='ready'||m.type==='failed'),{},{timeout:180000});
  const failed=await page.evaluate(()=>window.messages.find(m=>m.type==='failed'));assert(!failed,JSON.stringify(failed));
  const call=(action,args={})=>page.evaluate(([a,p])=>window.rpc(a,p),[action,args]);
  const catalog=await call('catalog');console.log(kind,'commands',catalog.length);
  if(kind==='word'){await call('command',{command:'text.insert',params:{text:'MCP Win — document français de validation'}});assert((await call('state')).dirty);}
  if(kind==='grid'){const imported=fs.readFileSync(path.join(out,'import.ods'));await page.evaluate(bytes=>window.rpc('open',{name:'import.ods',save_name:'converted.xlsx',bytes:new Uint8Array(bytes).buffer}),Array.from(imported));assert.equal((await call('command',{command:'cell.get',params:{cell:'A1'}})).value,9);await call('command',{command:'cell.set',params:{cell:'A1',value:7}});await call('command',{command:'cell.set',params:{cell:'B1',value:'=A1*6'}});assert.equal((await call('command',{command:'cell.get',params:{cell:'B1'}})).value,42);}
  if(kind==='deck'){await call('command',{command:'slide.new',params:{layout:'titleAndContent',title:'Validation MCP Win',body:'Une présentation locale.'}});assert((await call('state')).dirty);}
  if(kind==='design'){await call('command',{command:'frame.create',params:{rect:[72,72,400,200],content:'text',text:'Composition MCP Win'}});assert((await call('state')).dirty);}
  if(kind==='pdf'){const file=fs.readFileSync(path.join(out,'sample.pdf'));await page.evaluate(bytes=>window.rpc('open',{name:'validation.pdf',bytes:new Uint8Array(bytes).buffer}),Array.from(file));await call('save',{name:'validation.pdf'});await page.waitForFunction(()=>window.messages.some(m=>m.type==='save'));const pdf=await page.evaluate(()=>window.messages.find(m=>m.type==='save'));assert(Buffer.from(pdf.bytes).subarray(0,5).equals(Buffer.from('%PDF-')));}
  else{const ext={word:'docx',grid:'xlsx',deck:'pptx',design:'designcraft'}[kind];const before=await call('state');await call('save',{name:'validation.'+ext});await page.waitForFunction(()=>window.messages.some(m=>m.type==='save'));const saved=await page.evaluate(()=>window.messages.find(m=>m.type==='save'));assert(saved.bytes.length>100);fs.writeFileSync(path.join(out,kind+'.'+ext),Buffer.from(saved.bytes));if(kind==='word'){await call('command',{command:'text.insert',params:{text:'A modification made while saving.'}});await call('saved',{...before});assert((await call('state')).dirty,'Stale save acknowledgement must preserve newer edits');}}
  await page.waitForTimeout(1500);await page.screenshot({path:path.join(out,kind+'.png')});console.log('PASS',kind);
 }
 assert.deepEqual(errors,[]);console.log('Embedded editors: startup, commands, export and PDF open verified.');
 }finally{await browser.close();server.close()}
})().catch(e=>{console.error(e);server.close();process.exitCode=1});

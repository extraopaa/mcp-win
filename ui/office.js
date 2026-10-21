/* Local office host. Only messages from a known, same-origin editor frame are accepted. */
const officeDocuments=new Map();let officeActive=null,officeRequest=0;
const officePending=new Map();
const officeNewButton=document.createElement('button');officeNewButton.className='btn secondary';officeNewButton.textContent='Nouveau';officeNewButton.id='office-new';$('office').querySelector('.office-actions').prepend(officeNewButton);officeNewButton.addEventListener('click',()=>act(async()=>officeAttach(await invoke('office_new',{kind:officeDocuments.get(officeActive)?.kind||'word'}))));
function officeBytes(encoded){const text=atob(encoded),bytes=new Uint8Array(text.length);for(let i=0;i<text.length;i++)bytes[i]=text.charCodeAt(i);return bytes;}
function officeBase64(bytes){let text='';for(let i=0;i<bytes.length;i+=32768)text+=String.fromCharCode(...bytes.subarray(i,i+32768));return btoa(text);}
function officeRender(){
 const active=officeDocuments.get(officeActive),labels={word:'Texte',grid:'Tableur',deck:'Présentation',pdf:'PDF',design:'Composition'};
 document.querySelectorAll('[data-office-editor]').forEach(b=>b.classList.toggle('active',!$('office').hidden&&b.dataset.officeEditor===active?.kind));
 $('office').querySelector('.page-title').textContent=labels[active?.kind]||'Documents';if(!$('office').hidden)$('breadcrumb').textContent=labels[active?.kind]||'Documents';
 const has=officeDocuments.size>0;$('office-workspace').hidden=!has;$('office-welcome').hidden=has;
 $('office-save').disabled=!has||!!officeDocuments.get(officeActive)?.saving;$('office-save-as').disabled=$('office-save').disabled;
 $('office-ocr').hidden=officeDocuments.get(officeActive)?.kind!=='pdf';
 $('office-tabs').replaceChildren();
 for(const[id,d]of officeDocuments){d.frame.hidden=id!==officeActive;const tab=document.createElement('div');tab.className='office-tab'+(id===officeActive?' active':'');tab.setAttribute('role','tab');tab.setAttribute('aria-selected',String(id===officeActive));
  const title=document.createElement('button');title.className='office-tab-title';title.textContent=d.name+(d.dirty?' •':'');title.title=d.name;title.addEventListener('click',()=>{officeActive=id;officeRender()});
  const close=document.createElement('button');close.textContent='×';close.setAttribute('aria-label','Fermer '+d.name);close.addEventListener('click',()=>officeClose(id));tab.append(title,close);$('office-tabs').append(tab);
 }
}
function officeSend(d,action,params={},bytes){
 return new Promise((resolve,reject)=>{const id=++officeRequest,timer=setTimeout(()=>{officePending.delete(id);reject(new Error('L’éditeur ne répond pas. Le document reste ouvert.'));},120000);officePending.set(id,{document:d,resolve,reject,timer});
  const message={channel:'mcp-win-office',id,action,...params};if(bytes){message.bytes=bytes.buffer;d.frame.contentWindow.postMessage(message,location.origin,[bytes.buffer]);}else d.frame.contentWindow.postMessage(message,location.origin);
 });
}
async function officeAttach(file){
 if(!file)return null;switchTab('office');$('breadcrumb').textContent='Documents';
 const frame=document.createElement('iframe');frame.title=file.name;frame.setAttribute('sandbox','allow-scripts allow-same-origin allow-modals allow-downloads');frame.allow='clipboard-read; clipboard-write';
 const d={...file,frame,ready:false,dirty:false,saving:false,pendingSaveAs:false,state:{},bindings:new Map(),opening:true};delete d.data;
 officeDocuments.set(file.ticket,d);officeActive=file.ticket;
 const ready=new Promise((resolve,reject)=>{d.loaded=resolve;d.failed=reject;d.loadTimer=setTimeout(()=>reject(new Error('L’éditeur ne démarre pas.')),120000)});
 frame.src='office/editor.html?kind='+encodeURIComponent(file.kind);$('office-frames').append(frame);officeRender();
 try{await ready;if(file.data)await officeSend(d,'open',{name:file.name,save_name:file.save_name||file.name},officeBytes(file.data));d.opening=false;d.state=await officeSend(d,'state');d.engineDocument=d.state.document;d.bindings.set(String(d.engineDocument),{ticket:file.ticket,name:file.name});return d;}catch(e){clearTimeout(d.loadTimer);d.opening=false;d.frame.remove();officeDocuments.delete(file.ticket);await invoke('office_close',{ticket:file.ticket});if(officeActive===file.ticket)officeActive=officeDocuments.keys().next().value||null;officeRender();throw e;}
}
async function officeConfirm(d){
 if($('office-unsaved').open)return 'cancel';
 $('office-unsaved-name').textContent=d.name;$('office-unsaved').showModal();
 return new Promise(resolve=>{const done=answer=>{for(const[id,fn]of handlers)$(id).removeEventListener('click',fn);$('office-unsaved').removeEventListener('cancel',cancel);$('office-unsaved').close();resolve(answer)};
  const handlers=[['office-cancel-close',()=>done('cancel')],['office-discard',()=>done('discard')],['office-keep',()=>done('save')]],cancel=e=>{e.preventDefault();done('cancel')};for(const[id,fn]of handlers)$(id).addEventListener('click',fn);$('office-unsaved').addEventListener('cancel',cancel);
 });
}
async function officeClose(id){
 const d=officeDocuments.get(id);if(!d||d.saving)return;
 if(d.dirty){const choice=await officeConfirm(d);if(choice==='cancel')return;if(choice==='save'&&(!(await officeSave(d))||d.dirty))return;}
 for(const[id,p]of officePending)if(p.document===d){clearTimeout(p.timer);p.reject(new Error('Document fermé'));officePending.delete(id)}
 for(const binding of d.bindings.values())await invoke('office_close',{ticket:binding.ticket});if(!d.bindings.size)await invoke('office_close',{ticket:d.ticket});
 d.frame.remove();officeDocuments.delete(id);if(officeActive===id)officeActive=officeDocuments.keys().next().value||null;officeRender();
}
async function officeSave(d=officeDocuments.get(officeActive),saveAs=false){
 if(!d||!d.ready||d.saving)return false;d.pendingSaveAs=saveAs;d.saving=true;officeRender();
 const finished=new Promise(resolve=>{d.saved=resolve;d.saveTimer=setTimeout(()=>{d.saving=false;d.unsavedOnDisk=true;d.dirty=true;resolve(false);officeRender();toast('L’enregistrement n’a pas abouti. Le document reste ouvert.');},120000)});
 try{d.state=await officeSend(d,'state');d.saveRevision=d.state.revision;d.saveDocument=d.state.document;await officeSend(d,'save',{name:d.save_name||d.name});return await finished;}catch(e){clearTimeout(d.saveTimer);d.saving=false;d.saved?.(false);toast(e.message||e);officeRender();return false;}
}
async function officeStore(d,m){
 if(!(m.bytes instanceof ArrayBuffer)||m.bytes.byteLength>100*1024*1024)return;clearTimeout(d.saveTimer);
 const revision=d.saveRevision??d.state.revision,documentId=d.saveDocument??d.state.document;
 const binding=d.bindings.get(String(documentId))||{ticket:d.ticket,name:d.name};
 d.saving=true;officeRender();
 try{
  const result=await invoke('office_save',{ticket:binding.ticket,name:String(m.name||d.name),data:officeBase64(new Uint8Array(m.bytes)),saveAs:!!d.pendingSaveAs||!!m.saveAs});
  if(result.cancelled){d.unsavedOnDisk=true;d.dirty=true;d.saved?.(false);return;}
  const exported=!result.bound;
  if(!exported){d.unsavedOnDisk=false;binding.name=result.name;d.bindings.set(String(documentId),binding);d.name=result.name;d.save_name=result.name;d.frame.title=result.name;d.dirty=!!d.state.dirty&&d.state.revision!==revision;await officeSend(d,'saved',{revision,document:documentId,name:result.name});d.state=await officeSend(d,'state');d.dirty=!!d.state.dirty;}
  d.saved?.(!exported);toast('Fichier enregistré.');
 }catch(e){d.unsavedOnDisk=true;d.dirty=true;d.saved?.(false);toast(e);}finally{d.saving=false;d.pendingSaveAs=false;d.saveRevision=undefined;d.saveDocument=undefined;d.saved=null;officeRender();}
}
window.addEventListener('message',event=>{void officeHandleMessage(event).catch(e=>toast(e.message||e));});
async function officeHandleMessage(event){
 if(event.origin!==location.origin)return;
 const d=[...officeDocuments.values()].find(d=>d.frame.contentWindow===event.source);if(!d)return;
 let m=event.data;try{if(typeof m==='string')m=JSON.parse(m)}catch{return}if(!m||typeof m!=='object')return;
 if(m.type==='ready'){d.ready=true;clearTimeout(d.loadTimer);d.loaded();}
 else if(m.type==='failed'){clearTimeout(d.loadTimer);d.failed(new Error('L’éditeur ne peut pas démarrer : '+String(m.error||'')));}
 else if(m.type==='reply'){const p=officePending.get(m.id);if(!p||p.document!==d)return;officePending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(new Error(m.error)):p.resolve(m.result);}
 else if(m.type==='state'){
  d.state=m.state||{};d.dirty=!!d.state.dirty||!!d.unsavedOnDisk;
  if(!d.opening&&d.state.document!==undefined&&d.state.document!==d.engineDocument){
   d.engineDocument=d.state.document;let binding=d.bindings.get(String(d.engineDocument));
   if(!binding){try{const file=await invoke('office_new',{kind:d.kind});binding={ticket:file.ticket,name:file.name};d.bindings.set(String(d.engineDocument),binding)}catch(e){toast(e);return;}}
   d.name=binding.name;
  }officeRender();
 }else if(m.type==='save')await officeStore(d,m);
 else if(m.type==='ocr'){$('office-ocr-dialog').showModal();}
 else if(m.type==='navigation'){
  if(m.action==='new')await officeAttach(await invoke('office_new',{kind:d.kind}));
  else if(m.action==='open')await officeAttach(await invoke('office_pick'));
  else if(m.action==='close')await officeClose([...officeDocuments.entries()].find(([,v])=>v===d)?.[0]);
 }
 else if(m.type==='pick'){
  if(['picture','place','image'].includes(m.purpose)){const asset=await invoke('office_asset');if(asset)await officeSend(d,'asset',{name:asset.name,purpose:m.purpose,request:m.request,target:m.target},officeBytes(asset.data));}
  else {const file=await invoke('office_pick');if(file)await officeAttach(file);}
 }
}
document.querySelectorAll('[data-office-new]').forEach(b=>b.addEventListener('click',()=>act(async()=>officeAttach(await invoke('office_new',{kind:b.dataset.officeNew})))));
$('office-open').addEventListener('click',()=>act(async()=>officeAttach(await invoke('office_pick'))));
$('office-save').addEventListener('click',()=>officeSave());$('office-save-as').addEventListener('click',()=>officeSave(undefined,true));
$('office-ocr').addEventListener('click',()=>$('office-ocr-dialog').showModal());
$('office-close-ocr').addEventListener('click',()=>$('office-ocr-dialog').close());
$('office-copy-ocr').addEventListener('click',()=>copy($('office-ocr-text').value));
$('office-run-ocr').addEventListener('click',async()=>{const d=officeDocuments.get(officeActive);if(!d)return;$('office-run-ocr').disabled=true;$('office-ocr-status').textContent='Reconnaissance en cours…';try{if(d.dirty)throw new Error('Enregistrez le PDF avant de reconnaître le texte.');const r=await invoke('office_ocr',{ticket:(d.bindings.get(String(d.state.document))?.ticket||d.ticket),page:Number($('office-ocr-page').value)});$('office-ocr-text').value=r.text||'';$('office-copy-ocr').disabled=!r.text;$('office-ocr-status').textContent=r.needs_review?'Relisez le texte : la reconnaissance est incertaine.':'Texte reconnu.';}catch(e){$('office-ocr-status').textContent=String(e)}finally{$('office-run-ocr').disabled=false;}});
document.addEventListener('keydown',e=>{if((e.ctrlKey||e.metaKey)&&e.key.toLowerCase()==='s'&&!$('office').hidden){e.preventDefault();officeSave(undefined,e.shiftKey)}});
window.addEventListener('beforeunload',e=>{if([...officeDocuments.values()].some(d=>d.dirty||d.saving)){e.preventDefault();e.returnValue='';}});
document.querySelectorAll('[data-office-editor]').forEach(b=>b.addEventListener('click',()=>act(async()=>{
 const found=[...officeDocuments.entries()].find(([id,d])=>d.kind===b.dataset.officeEditor);
 if(found){officeActive=found[0];switchTab('office');officeRender();}
 else await officeAttach(await invoke('office_new',{kind:b.dataset.officeEditor}));
})));
let officeClosing=false;
setInterval(async()=>{try{
 const dirty=[...officeDocuments.values()].filter(d=>d.dirty||d.saving).map(d=>d.bindings.get(String(d.state.document))?.ticket||d.ticket);
 const requested=await invoke('office_lifecycle',{dirty});
 if(requested&&!officeClosing){officeClosing=true;try{if([...officeDocuments.values()].some(d=>d.saving)){toast('Attendez la fin de l’enregistrement avant de fermer l’application.');return;}for(const d of officeDocuments.values()){if(!d.dirty)continue;const choice=await officeConfirm(d);if(choice==='cancel')return;if(choice==='save'&&(!(await officeSave(d))||d.dirty))return;}await invoke('office_close_app');}finally{officeClosing=false;}}
}catch{}},750);
// Used by the native integration checks; no remote script is loaded.
window.MCPWinOffice={newDocument:async kind=>officeAttach(await invoke('office_new',{kind})),openShared:async(folder,path)=>officeAttach(await invoke('office_open_shared',{folder,path})),command:(command,params={})=>officeSend(officeDocuments.get(officeActive),'command',{command,params}),state:()=>officeSend(officeDocuments.get(officeActive),'state'),save:officeSave,close:()=>officeClose(officeActive),get active(){return officeDocuments.get(officeActive)}};

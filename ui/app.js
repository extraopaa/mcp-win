const $ = id => document.getElementById(id);
const invoke = (name, args) => window.__TAURI__.core.invoke(name, args);
let current = null, initialized = false, dirty = false, busy = false;
function toast(message) { $('toast').textContent = String(message); $('toast').hidden = false; }
function switchTab(name) { document.querySelectorAll('.page').forEach(p => p.hidden = p.id !== name); document.querySelectorAll('.nav').forEach(b => b.classList.toggle('active', b.dataset.tab === name)); $('breadcrumb').textContent = 'Espace de travail / ' + ({overview:"Vue d'ensemble",connection:'Connexion extérieure',activity:'Activité'})[name]; window.scrollTo(0,0); }
document.querySelectorAll('.nav').forEach(button => button.addEventListener('click', () => switchTab(button.dataset.tab)));
$('go-connection').addEventListener('click', () => switchTab('connection'));
function modeFields() { const mode = document.querySelector('[name=mode]:checked').value; $('lt-settings').hidden = mode !== 'cloudflare'; $('frp-settings').hidden = mode !== 'frp'; const auth=$('require-auth').checked; $('token-field').hidden=!auth; $('new-token').hidden=!auth; }
document.querySelectorAll('input,textarea').forEach(input => input.addEventListener('input', () => { dirty = true; modeFields(); }));
function fillSettings(c) { $('require-auth').checked=!!c.require_auth; document.querySelector(`[name=mode][value="${c.mode}"]`).checked = true; for (const [id,key] of [['relay','relay'],['port','port'],['token','token'],['frp-server','frp_server'],['frp-port','frp_port'],['frp-token','frp_token'],['public-url','public_url'],['frp-ca','frp_ca_pem'],['frp-tls-name','frp_tls_name']]) $(id).value = c[key]; modeFields(); }
function connection() { return {mode:document.querySelector('[name=mode]:checked').value,port:Number($('port').value),require_auth:true,token:$('token').value,relay:$('relay').value,frp_server:$('frp-server').value,frp_port:Number($('frp-port').value),frp_token:$('frp-token').value,public_url:$('public-url').value,frp_ca_pem:$('frp-ca').value,frp_tls_name:$('frp-tls-name').value}; }
async function save() { await invoke('save_settings',{connection:connection()}); dirty = false; }
async function act(action) { try { await action(); await refresh(); } catch(error) { toast(error); } }
$('save-settings').addEventListener('click', () => act(async () => {await save();toast('Réglages enregistrés.');}));
$('new-token').addEventListener('click', () => { $('token').value = crypto.randomUUID().replaceAll('-',''); dirty = true; toast('Nouveau jeton généré. Enregistrez les réglages pour l’appliquer.'); });
$('show-token').addEventListener('click', () => $('token').type = $('token').type === 'password' ? 'text' : 'password');
$('add-folder').addEventListener('click', () => act(() => invoke('add_folder')));
$('toggle-server').addEventListener('click', async () => { if(busy && !current?.running) return; busy = true; try { if(current?.running) await invoke('stop_mcp'); else { if(dirty) await save(); const pending = invoke('start_mcp'); await refresh(); await pending; } } catch(error) { toast(error); } finally {busy=false;await refresh();} });
async function copy(text, message) { if(!text) return toast('Démarrez le serveur pour obtenir son adresse.'); try { await navigator.clipboard.writeText(text); toast(message || 'Copié dans le presse-papiers.'); } catch { toast('Le presse-papiers est indisponible.'); } }
$('copy-address').addEventListener('click', () => copy(current?.address,'Adresse publique copiée.'));
$('copy-token').addEventListener('click', () => copy(current?.connection.token,'Jeton copié.'));
$('copy-bearer').addEventListener('click', () => { const token=current?.connection.token; if(token) copy(`Bearer ${token}`,'Bearer et jeton copiés.'); });
$('copy-config').addEventListener('click', () => { const token=current?.connection.token; if(token) copy(JSON.stringify({Authorization:`Bearer ${token}`},null,2),'Headers sécurisés JSON copiés.'); });
$('close-preview').addEventListener('click', () => $('preview').close());
async function preview(folder) { $('preview-text').textContent='Choisissez un fichier dans la boîte de dialogue…';$('preview-image').hidden=true;$('preview').showModal();try { const value=await invoke('preview_file',{folder});if(!value){$('preview').close();return;} $('preview-text').textContent=JSON.stringify({...value,png:undefined},null,2);if(value.png){$('preview-image').src=value.png;$('preview-image').hidden=false;} }catch(error){$('preview-text').textContent=String(error);} }
function render(data) {
  current=data;if(!initialized){fillSettings(data.connection);initialized=true;}
  const reconnecting=data.ready&&data.connection.mode==='localtunnel'&&data.active_connections===0;
  $('status-dot').classList.toggle('live',data.ready&&!reconnecting);
  $('status-badge').textContent=data.running?(data.ready?(reconnecting?'RECONNEXION':'EN LIGNE'):'CONNEXION EN COURS'):'HORS LIGNE';
  $('status-title').textContent=data.ready?(reconnecting?'Le relais se reconnecte…':data.connection.mode==='local'?'Votre MCP est disponible sur ce PC':'Votre MCP est ouvert vers l’extérieur'):data.running?'Ouverture du tunnel public…':'Prêt à ouvrir la connexion';
  $('status-description').textContent=data.ready?(data.connection.require_auth?'Seuls vos dossiers partagés sont accessibles, avec votre jeton.':'Accès sans authentification aux dossiers partagés : l’adresse publique suffit.'):data.running?'La connexion au relais peut prendre quelques secondes.':'Un clic pour démarrer le MCP et votre tunnel public.';
  $('toggle-server').textContent=data.running?'Arrêter le MCP':'Démarrer le MCP →';
  $('public-address').textContent=data.address||'Votre adresse HTTPS apparaîtra ici';
  $('connection-address').textContent=data.address||'Démarrez le MCP pour obtenir l’adresse';
  $('copy-address').disabled=!data.address; $('auth-label').textContent=data.connection.require_auth?'Accès par jeton':'Sans authentification'; $('auth-description').textContent=data.connection.require_auth?'Authorization: Bearer <votre jeton>':'Aucune — l’adresse suffit'; $('copy-token').hidden=!data.connection.require_auth;
  $('mode-label').textContent={cloudflare:'◉ Cloudflare · tunnel chiffré',frp:'◉ Votre relais personnel',local:'◉ Local uniquement'}[data.connection.mode];
  $('add-folder').disabled=data.running;
  document.querySelectorAll('#connection input,#connection textarea,#save-settings,#new-token').forEach(e=>e.disabled=data.running||e.id==='require-auth');
  $('folder-count').textContent=data.folders.length;
  const folders=$('folders');
  const key=JSON.stringify([data.folders,data.running]);
  if(folders.dataset.key!==key){folders.dataset.key=key;folders.replaceChildren();if(!data.folders.length){const empty=document.createElement('div');empty.className='empty';empty.innerHTML='<div class="empty-icon">▱</div><h3>Un espace pour vos documents</h3><p>Choisissez un dossier de votre ordinateur. Vous gardez le contrôle.</p><button class="secondary">+ Choisir mon premier dossier</button>';empty.querySelector('button').addEventListener('click',()=>act(()=>invoke('add_folder')));folders.append(empty);}for(const f of data.folders){const row=document.createElement('div');row.className='folder-row';row.innerHTML='<span class="folder-icon">▱</span><div class="folder-info"><strong></strong><p></p></div><div class="folder-actions"><button class="secondary">Aperçu</button><button class="secondary">Retirer</button><button class="secondary write-permission">Lecture seule</button><button class="secondary history-button">Historique</button></div>';row.querySelector('strong').textContent=f.name;row.querySelector('p').textContent=f.path;const buttons=row.querySelectorAll('button');buttons[0].addEventListener('click',()=>preview(f.id));buttons[1].disabled=data.running;buttons[1].addEventListener('click',()=>act(()=>invoke('remove_folder',{id:f.id})));buttons[2].textContent=f.writable?'Écriture autorisée':'Lecture seule';buttons[2].disabled=data.running;buttons[2].addEventListener('click',()=>act(()=>invoke('set_folder_writable',{id:f.id,writable:!f.writable})));buttons[3].addEventListener('click',()=>openHistory(f.id));folders.append(row);}}
  const logs=$('logs');logs.replaceChildren();if(!data.logs.length){logs.textContent='Les connexions et lectures apparaîtront ici.';}for(const line of data.logs){const el=document.createElement('div');el.className='log-line';el.textContent=line;logs.append(el);}
}
async function refresh(){try{render(await invoke('snapshot'));}catch(error){if(!initialized)toast(`Connexion à l’application impossible : ${error}`);}}
refresh();setInterval(refresh,1000);

$('close-history').addEventListener('click',()=>$('history-dialog').close());
async function openHistory(folder){
 const dialog=$('history-dialog');if(!dialog.open)dialog.showModal();const content=$('history-versions');content.textContent='Chargement…';
 try{
  const data=await invoke('list_history',{folder});content.replaceChildren();
  if(!data.versions.length){content.textContent='Aucune version enregistrée. Une copie sera conservée avant la prochaine modification d’un fichier existant.';return;}
  for(const version of data.versions){
   const row=document.createElement('div');row.className='history-row';const info=document.createElement('div');const name=document.createElement('strong');name.textContent=version.path;const date=document.createElement('p');date.textContent=new Date(version.created_at_ms||version.timestamp*1000).toLocaleString('fr-FR')+' · '+version.bytes+' octets';info.append(name,date);
   const button=document.createElement('button');button.className='secondary';button.textContent='Restaurer cette version';button.disabled=!!current?.running;
   button.addEventListener('click',async()=>{button.disabled=true;try{await invoke('restore_version',{folder,version:version.id});toast('Version restaurée. L’état remplacé est conservé dans l’historique.');await openHistory(folder);}catch(e){toast(e);button.disabled=!!current?.running;}});row.append(info,button);content.append(row);
  }
 }catch(e){content.textContent=String(e);}
}

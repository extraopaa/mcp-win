const $ = id => document.getElementById(id);
const invoke = (name, args) => window.__TAURI__.core.invoke(name, args);
let current = null, initialized = false, dirty = false, busy = false;
function toast(message) { $('toast').textContent = String(message); $('toast').hidden = false; }
function switchTab(name) { document.querySelectorAll('.page').forEach(p => p.hidden = p.id !== name); document.querySelectorAll('.nav').forEach(b => b.classList.toggle('active', b.dataset.tab === name)); $('breadcrumb').textContent = 'Espace de travail / ' + ({overview:"Vue d'ensemble",connection:'Connexion extérieure',activity:'Activité'})[name]; window.scrollTo(0,0); }
document.querySelectorAll('.nav').forEach(button => button.addEventListener('click', () => switchTab(button.dataset.tab)));
$('go-connection').addEventListener('click', () => switchTab('connection'));
function modeFields() { const mode = document.querySelector('[name=mode]:checked').value; $('lt-settings').hidden = mode !== 'localtunnel'; $('frp-settings').hidden = mode !== 'frp'; const auth=$('require-auth').checked; $('token-field').hidden=!auth; $('new-token').hidden=!auth; }
document.querySelectorAll('input').forEach(input => input.addEventListener('input', () => { dirty = true; modeFields(); }));
function fillSettings(c) { $('require-auth').checked=!!c.require_auth; document.querySelector(`[name=mode][value="${c.mode}"]`).checked = true; for (const [id,key] of [['relay','relay'],['port','port'],['token','token'],['frp-server','frp_server'],['frp-port','frp_port'],['frp-token','frp_token'],['public-url','public_url']]) $(id).value = c[key]; modeFields(); }
function connection() { return {mode:document.querySelector('[name=mode]:checked').value,port:Number($('port').value),require_auth:$('require-auth').checked,token:$('token').value,relay:$('relay').value,frp_server:$('frp-server').value,frp_port:Number($('frp-port').value),frp_token:$('frp-token').value,public_url:$('public-url').value}; }
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
$('copy-config').addEventListener('click', () => { if(!current?.address) return toast('Démarrez le MCP pour obtenir son adresse publique.'); const headers = {}; if(current.connection.require_auth) headers.Authorization=`Bearer ${current.connection.token}`; if(current.connection.mode==='localtunnel') headers['bypass-tunnel-reminder']='true'; copy(JSON.stringify({mcpServers:{'mcp-win':{url:current.address,headers}}},null,2),'Configuration copiée.'); });
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
  $('mode-label').textContent={localtunnel:'◉ Tunnel public gratuit',frp:'◉ Votre relais personnel',local:'◉ Local uniquement'}[data.connection.mode];
  $('add-folder').disabled=data.running;
  document.querySelectorAll('#connection input,#save-settings,#new-token').forEach(e=>e.disabled=data.running);
  $('folder-count').textContent=data.folders.length;
  const folders=$('folders');
  const key=JSON.stringify([data.folders,data.running]);
  if(folders.dataset.key!==key){folders.dataset.key=key;folders.replaceChildren();if(!data.folders.length){const empty=document.createElement('div');empty.className='empty';empty.innerHTML='<div class="empty-icon">▱</div><h3>Un espace pour vos documents</h3><p>Choisissez un dossier de votre ordinateur. Vous gardez le contrôle.</p><button class="secondary">+ Choisir mon premier dossier</button>';empty.querySelector('button').addEventListener('click',()=>act(()=>invoke('add_folder')));folders.append(empty);}for(const f of data.folders){const row=document.createElement('div');row.className='folder-row';row.innerHTML='<span class="folder-icon">▱</span><div class="folder-info"><strong></strong><p></p></div><div class="folder-actions"><button class="secondary">Aperçu</button><button class="secondary">Retirer</button></div>';row.querySelector('strong').textContent=f.name;row.querySelector('p').textContent=f.path;const buttons=row.querySelectorAll('button');buttons[0].addEventListener('click',()=>preview(f.id));buttons[1].disabled=data.running;buttons[1].addEventListener('click',()=>act(()=>invoke('remove_folder',{id:f.id})));folders.append(row);}}
  const logs=$('logs');logs.replaceChildren();if(!data.logs.length){logs.textContent='Les connexions et lectures apparaîtront ici.';}for(const line of data.logs){const el=document.createElement('div');el.className='log-line';el.textContent=line;logs.append(el);}
}
async function refresh(){try{render(await invoke('snapshot'));}catch(error){if(!initialized)toast(`Connexion à l’application impossible : ${error}`);}}
refresh();setInterval(refresh,1000);

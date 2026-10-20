let updateState=null,updateInstalling=false;
function renderUpdate(value){
 if(!value)return;updateState=value;const status=value.status;
 $('update-current').textContent='Version installée · '+value.current_version;
 const available=['available','ready'].includes(status),working=['downloading','verifying','installing'].includes(status);
 $('update-label').textContent=available?'Mise à jour disponible':'Mises à jour';$('open-updates').classList.toggle('available',available);
 $('update-latest').hidden=!value.version;$('update-latest').textContent=value.version?'Version '+value.version:'';
 const messages={idle:'Recherche automatique au démarrage.',checking:'Recherche…',current:'Aucune mise à jour disponible.',available:'Une nouvelle version est disponible.',downloading:'Téléchargement…',verifying:'Vérification du paquet…',ready:'Paquet vérifié. Prêt à installer.',installing:'Redémarrage de MCP Win…',error:value.error||'Recherche impossible.'};
 $('update-message').textContent=messages[status]||'';
 $('update-hint').textContent=status==='ready'?(current?.running?'Arrêtez le serveur pour installer.':'Vos réglages et dossiers partagés seront conservés.'):status==='downloading'?'Vous pouvez continuer à utiliser l’application.':'';
 const progress=$('update-progress');progress.hidden=!working;
 if(status==='verifying')progress.removeAttribute('value');else progress.value=value.progress||0;
 $('update-transfer').hidden=status!=='downloading'||!value.total_bytes;
 $('update-transfer').textContent=`${((value.downloaded_bytes||0)/1048576).toFixed(1)} / ${((value.total_bytes||0)/1048576).toFixed(1)} Mio · ${value.progress||0}%`;
 $('update-steps').hidden=!working&&status!=='ready';
 const step=status==='downloading'?0:status==='verifying'?1:2;
 $('update-steps').querySelectorAll('li').forEach((e,i)=>{e.classList.toggle('done',i<step);e.classList.toggle('current',i===step)});
 $('update-last-check').textContent=value.last_check?'Vérifié à '+new Date(value.last_check*1000).toLocaleTimeString('fr-FR',{hour:'2-digit',minute:'2-digit'}):'';
 $('check-update').disabled=updateInstalling||working||status==='checking'||status==='ready';
 $('download-update').hidden=status!=='available';$('download-update').disabled=updateInstalling;
 $('install-update').hidden=status!=='ready';$('install-update').disabled=updateInstalling||!!current?.running;
}
async function checkUpdates(force=false){try{renderUpdate(await invoke('check_update',{force}));}catch(e){if(force)toast(e)}}
$('open-updates').addEventListener('click',()=>{$('update-dialog').showModal();checkUpdates(false)});
$('close-updates').addEventListener('click',()=>$('update-dialog').close());
$('check-update').addEventListener('click',()=>checkUpdates(true));
$('download-update').addEventListener('click',async()=>{if(updateInstalling)return;updateInstalling=true;renderUpdate(updateState);try{renderUpdate(await invoke('download_update'))}catch(e){toast(e)}finally{updateInstalling=false;renderUpdate(updateState)}});
$('install-update').addEventListener('click',async()=>{if(updateInstalling||current?.running||updateState?.status!=='ready')return;updateInstalling=true;renderUpdate(updateState);try{await invoke('install_update')}catch(e){toast(e)}finally{updateInstalling=false;renderUpdate(updateState)}});
setTimeout(()=>checkUpdates(false),5000);
setInterval(()=>checkUpdates(false),21600000);
setInterval(async()=>{if($('update-dialog').open||updateInstalling){try{renderUpdate(await invoke('update_status'))}catch{}}},1000);

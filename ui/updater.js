let updateState=null,updateInstalling=false;
function renderUpdate(value){
 if(!value)return;updateState=value;
 $('update-current').textContent='Version '+value.current_version;
 const available=['available','ready'].includes(value.status);
 $('update-label').textContent=available?'Mise à jour disponible':'Mises à jour';
 $('open-updates').classList.toggle('available',available);
 const messages={idle:'Recherche automatique au démarrage.',checking:'Recherche…',current:'Aucune mise à jour disponible.',available:`Version ${value.version} disponible.`,downloading:`Téléchargement… ${value.progress}%`,ready:`Version ${value.version} prête à installer.`,error:value.error||'Recherche impossible.'};
 $('update-message').textContent=messages[value.status]||'';
 if(available&&current?.running)$('update-message').textContent+=' Arrêtez le serveur pour installer.';
 $('update-progress').hidden=value.status!=='downloading';$('update-progress').value=value.progress;
 $('check-update').disabled=updateInstalling||['checking','downloading','ready'].includes(value.status);
 $('install-update').hidden=!available;$('install-update').disabled=updateInstalling||!!current?.running;
}
async function checkUpdates(force=false){try{renderUpdate(await invoke('check_update',{force}));}catch(e){if(force)toast(e)}}
$('open-updates').addEventListener('click',()=>{$('update-dialog').showModal();checkUpdates(false)});
$('close-updates').addEventListener('click',()=>$('update-dialog').close());
$('check-update').addEventListener('click',()=>checkUpdates(true));
$('install-update').addEventListener('click',async()=>{
 if(updateInstalling||current?.running)return;updateInstalling=true;renderUpdate(updateState);
 try{
  if(updateState?.status!=='ready')renderUpdate(await invoke('download_update'));
  if(updateState?.status==='ready')await invoke('install_update');
 }catch(e){toast(e)}finally{updateInstalling=false;renderUpdate(updateState)}
});
setTimeout(()=>checkUpdates(false),5000);
setInterval(()=>checkUpdates(false),21600000);
setInterval(async()=>{if($('update-dialog').open||updateInstalling){try{renderUpdate(await invoke('update_status'))}catch{}}},1000);

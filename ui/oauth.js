let oauthNotified=new Set();
async function refreshOAuth(){
 const active=current?.connection?.auth_mode==='oauth'&&current?.running;
 if(!active){$('oauth-badge').hidden=true;$('oauth-requests').textContent='Serveur arrêté.';$('oauth-clients').replaceChildren();oauthNotified.clear();return;}
 try{
  const value=await invoke('oauth_status');if(!value)return;
  const requests=value.requests||[],clients=value.clients||[];
  $('oauth-badge').hidden=!requests.length;$('oauth-badge').textContent=requests.length;
  const pending=$('oauth-requests'),approved=$('oauth-clients');pending.replaceChildren();approved.replaceChildren();
  for(const r of requests){
   const row=document.createElement('div');row.className='oauth-row';
   row.innerHTML=`<strong>${safe(r.name||'Assistant IA')}</strong><p class="oauth-redirect">${safe(r.redirect_uri)}</p><div class="form-actions"></div>`;
   for(const[action,label]of[['approve','Autoriser'],['deny','Refuser']]){
    const b=document.createElement('button');b.className=action==='approve'?'btn primary small':'btn secondary small';b.textContent=label;
    b.addEventListener('click',async()=>{b.disabled=true;try{await invoke('oauth_decision',{id:r.id,action});await refreshOAuth()}catch(e){toast(e)}});row.querySelector('.form-actions').append(b);
   }pending.append(row);
   if(!oauthNotified.has(r.id)){if(oauthNotified.size>256)oauthNotified.delete(oauthNotified.values().next().value);oauthNotified.add(r.id);toast('Une connexion OAuth attend votre accord dans Connexion IA.');}
  }
  if(!requests.length)pending.textContent='Aucune demande.';
  for(const c of clients){
   const row=document.createElement('div');row.className='oauth-row studio-row';const name=document.createElement('strong');name.textContent=c.name;
   const b=document.createElement('button');b.className='btn secondary small';b.textContent='Révoquer';b.addEventListener('click',async()=>{b.disabled=true;try{await invoke('oauth_decision',{id:c.id,action:'revoke'});await refreshOAuth()}catch(e){toast(e)}});row.append(name,b);approved.append(row);
  }
  if(!clients.length)approved.textContent='Aucun assistant autorisé.';
 }catch(e){if(!$('connection').hidden)toast(e)}
}
setInterval(refreshOAuth,2000);
document.querySelector('[data-tab=connection]').addEventListener('click',refreshOAuth);

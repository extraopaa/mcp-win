// Vérification de la nouvelle UI avec IPC simulé, jamais de documents personnels.
const {chromium} = require('../tools/dev/node_modules/playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const root = path.resolve(__dirname, '../ui');
const output = path.resolve(__dirname, '../target/ui-design');
const csp = JSON.parse(fs.readFileSync(path.join(root,'../tauri.conf.json'),'utf8')).app.security.csp;
const server = http.createServer((req, res) => {
  if (req.url === '/favicon.ico') {res.writeHead(204).end(); return;}
  const name = req.url === '/' ? 'index.html' : req.url.slice(1);
  if (!['index.html','style.css','office.css','office.js','icons.js','app.js','studio.js','updater.js','oauth.js','brand/icon.png','brand/favicon.png'].includes(name)) {res.writeHead(404).end(); return;}
  res.writeHead(200, {'Content-Type':name.endsWith('.png')?'image/png':name.endsWith('.html')?'text/html; charset=utf-8':name.endsWith('.css')?'text/css; charset=utf-8':'text/javascript; charset=utf-8','Content-Security-Policy':csp});
  res.end(fs.readFileSync(path.join(root, name)));
});
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({channel:'msedge',headless:true});
  try {
    fs.mkdirSync(output, {recursive:true});
    const page = await browser.newPage({viewport:{width:1440,height:1210}});
    const errors = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('console', m => {if (m.type() === 'error') errors.push(m.text());});
    await page.addInitScript(() => {
      window.calls = [];
      window.testOAuth={requests:[{id:"auth-test",name:"Assistant <img src=x>",redirect_uri:"https://client.example/callback"}],clients:[]};
      window.testUpdate={status:'available',version:'1.1.0',current_version:'1.0.0',progress:0,error:null};
      window.testWorkspace={brands:{selected:'default',profiles:[{id:'default',name:'Professionnel',primary:'6366F1',secondary:'3B82F6',font:'Lato',organization:'',contact:'',footer:'',logo_base64:''}]},indexes:[{folder:'work',status:{documents:0}}],workflows:{jobs:[{id:'job-test',name:'Dossier de livraison',status:'paused',completed:1,steps:3,progress:33}]},reviews:{revisions:[{id:'revision-test',label:'Correction du tarif',path:'devis.docx',output_path:'devis.docx',status:'pending',diff:'- 100 euros\n+ 125 euros'}]}};
      window.testState = {
        connection:{mode:'cloudflare',port:8787,require_auth:true,token:'0123456789abcdef0123456789abcdef',relay:'',frp_server:'',frp_port:7000,frp_token:'',public_url:'',frp_ca_pem:'',frp_tls_name:''},
        running:true,ready:true,address:'https://documents-test.trycloudflare.com/mcp',
        folders:[{id:'work',name:'Documents professionnels',path:'C:\\Documents\\Travail',writable:false},{id:'projects',name:'Projets & rapports',path:'C:\\Documents\\Projets',writable:true},{id:'bills',name:'Factures 2026',path:'D:\\Archives\\Factures_2026',writable:false}],
        logs:['16:21:00 UTC  Lecture : rapport-annuel.pdf','16:19:00 UTC  Liste des dossiers partagés','16:17:00 UTC  Génération : synthese-octobre.docx','16:14:00 UTC  Requête refusée : authentification','16:12:00 UTC  Serveur démarré sur 127.0.0.1:8787','15:58:00 UTC  OCR : justificatif.pdf']
      };
      Object.defineProperty(navigator, 'clipboard', {value:{writeText:async text => window.copied = text}});
      window.__TAURI__ = {core:{invoke:async (name,args) => {
        window.calls.push({name,args});
        const s = window.testState;
        if (name === 'check_update' || name === 'update_status') return structuredClone(window.testUpdate);
        if (name === 'download_update') {window.testUpdate.status='ready';return structuredClone(window.testUpdate);}
        if (name === 'install_update') return;
        if (name === 'oauth_status') return structuredClone(window.testOAuth);
        if (name === 'oauth_decision') {if(args.action==='approve')window.testOAuth.clients.push({id:'client-test',name:'Assistant de test'});window.testOAuth.requests=[];if(args.action==='revoke')window.testOAuth.clients=[];return;}
        if (name === 'snapshot') return structuredClone(s);
        if (name === 'v1_workspace') return structuredClone(window.testWorkspace);
        if (name === 'save_brand_profiles') {window.testWorkspace.brands=structuredClone(args.profiles);return;}
        if (name === 'build_local_index') {window.testWorkspace.indexes[0].status.documents=1;return {indexed:1,unchanged:0,next_offset:null};}
        if (name === 'revision_action') {window.testWorkspace.reviews.revisions[0].status=args.action==='apply'?'applied':args.action==='undo'?'undone':'rejected';return;}
        if (name === 'workflow_action') {window.testWorkspace.workflows.jobs[0].status=args.action==='resume'?'running':args.action==='pause'?'paused':args.action==='undo'?'undone':'cancelled';return;}
        if (window.failCommand === name) throw 'Erreur synthétique de vérification';
        if (name === 'stop_mcp') {s.running=false;s.ready=false;s.address=null;}
        else if (name === 'start_mcp') {s.running=true;s.ready=true;s.address='https://documents-test.trycloudflare.com/mcp';}
        else if (name === 'save_settings') s.connection = structuredClone(args.connection);
        else if (name === 'add_folder') s.folders.push({id:'new',name:'Nouveau dossier',path:'C:\\Documents\\Nouveau',writable:false});
        else if (name === 'set_folder_writable') s.folders.find(f=>f.id===args.id).writable=args.writable;
        else if (name === 'remove_folder') s.folders=s.folders.filter(f=>f.id!==args.id);
        else if (name === 'preview_file') return {path:'rapport.txt',text:'Aperçu local vérifié'};
        else if (name === 'list_history') return {versions:[{id:'version-test',path:'rapport.txt',bytes:42,created_at_ms:1792440000000}]};
        else if (name === 'restore_version') window.restored=args;
      }}};
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.waitForFunction(() => document.getElementById('token').value.length === 32);
    await page.waitForFunction(() => document.querySelector('.brand-mark img').naturalWidth === 512);
    const linkPages = [['repository','overview'],['documentation','overview'],['getting-started','overview'],['connection-guide','connection'],['tools-reference','tools']];
    for (const [link, tab] of linkPages) {
      await page.locator(`[data-tab="${tab}"]`).click();
      const anchor = page.locator(`[data-external="${link}"]`);
      assert((await anchor.getAttribute('href')).startsWith('https://github.com/extraopaa/mcp-win'));
      await anchor.click();
      await page.waitForFunction(key => window.calls.some(c => c.name === 'open_external_link' && c.args.link === key), link);
    }
    assert.equal(page.context().pages().length,1,'La page externe ne remplace pas la WebView MCP');
    const tabs = ['overview','shared-folders','connection','activity','tools'];
    for (const tab of tabs) {
      await page.locator(`[data-tab="${tab}"]`).click();
      await page.waitForTimeout(300);
      assert.equal(await page.locator('.page:visible').count(),1);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, tab);
      await page.screenshot({path:path.join(output,`${tab}.png`), fullPage:true});
    }
    assert.equal(await page.locator('.tool-card').count(),9);
    await page.locator('.tool-card').first().click();
    assert.equal(await page.locator('#help-dialog code').count(),9);
    await page.keyboard.press('Escape');
    await page.locator('[data-tab=connection]').click();
    await page.locator('#copy-config').click();
    assert.deepEqual(JSON.parse(await page.evaluate(()=>window.copied)),{Authorization:'Bearer 0123456789abcdef0123456789abcdef'});
    await page.locator('#copy-bearer').click();
    assert.equal(await page.evaluate(()=>window.copied),'Bearer 0123456789abcdef0123456789abcdef');
    assert(await page.locator('[value=frp]').isDisabled());
    await page.locator('[data-tab=shared-folders]').click();
    assert(await page.locator('#add-folder').isDisabled());
    assert(await page.locator('.write-permission').first().isDisabled());
    await page.locator('.preview-button').first().click();
    await page.waitForFunction(()=>document.getElementById('preview-text').textContent.includes('Aperçu local vérifié'));
    await page.locator('#close-preview').click();
    await page.locator('.history-button').first().click();
    assert(await page.locator('.history-row button').isDisabled());
    await page.locator('#close-history').click();
    await page.locator('[data-tab=overview]').click();
    await page.locator('#toggle-server').click();
    await page.waitForFunction(()=>!window.testState.running);
    await page.locator('[data-tab=shared-folders]').click();
    await page.locator('.write-permission').first().click();
    await page.waitForFunction(()=>window.testState.folders[0].writable);
    await page.locator('#add-folder').click();
    await page.waitForFunction(()=>window.testState.folders.length===4);
    await page.locator('.remove-folder').last().click();
    await page.waitForFunction(()=>window.testState.folders.length===3);
    await page.locator('.history-button').first().click();
    await page.locator('.history-row button').click();
    await page.waitForFunction(()=>window.restored?.folder==='work');
    await page.locator('#close-history').click();
    await page.locator('[data-tab=connection]').click();
    await page.locator('[value=frp]').check();
    await page.locator('#frp-ca').fill('CERTIFICAT-DE-TEST');
    await page.locator('#frp-tls-name').fill('relay.example');
    await page.locator('#save-settings').click();
    await page.waitForFunction(()=>window.testState.connection.frp_tls_name==='relay.example');
    assert.equal(await page.evaluate(()=>window.testState.connection.frp_ca_pem),'CERTIFICAT-DE-TEST');
    await page.locator('#new-token').click();
    assert.notEqual(await page.locator('#token').inputValue(),'0123456789abcdef0123456789abcdef');
    await page.locator('#copy-bearer').click();
    assert.equal(await page.evaluate(()=>window.copied),'Bearer 0123456789abcdef0123456789abcdef','Un brouillon de jeton ne remplace pas le jeton actif');
    await page.locator('#save-settings').click();
    await page.waitForFunction(()=>window.testState.connection.token===document.getElementById('token').value);
    await page.locator('[data-tab=studio]').click();
    await page.waitForFunction(()=>document.getElementById('brand-name').value==='Professionnel');
    await page.locator('#brand-name').fill('Identité de démonstration');
    await page.locator('#brand-logo').setInputFiles(path.join(root,'brand/favicon.png'));
    await page.waitForFunction(()=>!document.getElementById('brand-logo-preview').hidden);
    await page.locator('#save-brands').click();
    await page.waitForFunction(()=>window.testWorkspace.brands.profiles[0].name==='Identité de démonstration');
    await page.screenshot({path:path.join(output,'studio-brands.png'),fullPage:true});
    await page.locator('[data-studio=index]').click();
    await page.locator('#studio-indexes button').first().click();
    await page.waitForFunction(()=>window.testWorkspace.indexes[0].status.documents===1);
    await page.locator('[data-studio=reviews]').click();
    await page.locator('.compare-revision').click();
    assert((await page.locator('.revision-diff').textContent()).includes('125 euros'));
    await page.keyboard.press('Escape');
    await page.locator('[data-revision-action=apply]').click();
    await page.waitForFunction(()=>window.testWorkspace.reviews.revisions[0].status==='applied');
    await page.locator('[data-revision-action=undo]').click();
    await page.waitForFunction(()=>window.testWorkspace.reviews.revisions[0].status==='undone');
    await page.locator('[data-studio=workflows]').click();
    assert(await page.locator('[data-workflow-action=resume]').isDisabled());
    await page.locator('[data-workflow-action=undo]').click();
    await page.waitForFunction(()=>window.testWorkspace.workflows.jobs[0].status==='undone');
    await page.evaluate(async()=>{window.testState.running=true;window.testState.ready=true;await refresh();});
    await page.locator('[data-studio=brands]').click();
    assert(await page.locator('#save-brands').isDisabled());
    await page.evaluate(async()=>{window.testState.running=false;window.testState.ready=false;await refresh();});
    await page.locator('[data-tab=connection]').click();await page.locator('#auth-mode').selectOption('oauth');
    assert(await page.locator('#token-settings').isHidden());await page.locator('#save-settings').click();
    await page.evaluate(async()=>{window.testState.running=true;await refresh();await refreshOAuth()});
    assert.equal(await page.locator('#oauth-requests img').count(),0);
    await page.locator('#oauth-requests button').filter({hasText:'Autoriser'}).click();
    await page.waitForFunction(()=>window.testOAuth.clients.length===1);
    await page.locator('#oauth-clients button').click();await page.waitForFunction(()=>window.testOAuth.clients.length===0);
    await page.evaluate(async()=>{window.testState.running=false;await refresh()});await page.locator('#auth-mode').selectOption('token');await page.locator('#save-settings').click();
    await page.locator('[data-tab=activity]').click();
    await page.locator('[data-filter=write]').click();
    assert.equal(await page.locator('.log-line').count(),1);
    await page.locator('[data-filter=all]').click();
    await page.locator('#activity-search').fill('justificatif');
    assert.equal(await page.locator('.log-line').count(),1);
    await page.locator('#activity-search').fill('<img src=x onerror=alert(1)>');
    assert.equal(await page.locator('.log-line').count(),0);
    await page.locator('#activity-search').fill('');
    // Noms et logs non fiables ne deviennent jamais du HTML exécutable.
    await page.evaluate(async()=>{window.testState.folders[0].name='<img src=x onerror=alert(1)>';window.testState.logs.unshift('16:22:00 UTC  Lecture : <img src=x onerror=alert(1)>');await refresh();});
    assert.equal(await page.locator('#logs img').count(),0);
    assert.equal(await page.locator('#folders img').count(),0);
    for (const width of [1180,900]) {
      await page.setViewportSize({width,height:820});
      for (const tab of [...tabs,'studio']) {
        await page.locator(`[data-tab="${tab}"]`).click();
        assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false,`${tab} à ${width}px`);
      }
    }
    await page.evaluate(async()=>{window.testState.folders=[];window.testState.logs=[];await refresh();});
    await page.locator('[data-tab=overview]').click();
    assert(await page.locator('#overview-folders .empty-state').isVisible());
    await page.locator('#overview-folders button').click();
    await page.waitForFunction(()=>window.testState.folders.length===1);
    await page.evaluate(()=>window.failCommand='start_mcp');
    await page.locator('#toggle-server').click();
    await page.waitForFunction(()=>document.getElementById('toast-text').textContent.includes('Erreur synthétique'));
    assert.equal(await page.locator('#toggle-server').isDisabled(),false);
    await page.locator('#open-updates').click();
    await page.waitForFunction(()=>document.getElementById('update-latest').textContent.includes('1.1.0'));
    await page.evaluate(async()=>{window.testState.running=true;await refresh();});
    await page.locator('#download-update').click();
    await page.waitForFunction(()=>window.testUpdate.status==='ready');
    await page.waitForFunction(()=>document.getElementById('install-update').disabled);
    await page.evaluate(async()=>{window.testState.running=false;await refresh();});
    await page.waitForFunction(()=>!document.getElementById('install-update').disabled);
    await page.locator('#install-update').click();
    await page.waitForFunction(()=>window.calls.some(c=>c.name==='install_update'));
    await page.evaluate(()=>{window.testUpdate.status='error';window.testUpdate.error='Signature de mise à jour invalide';renderUpdate(window.testUpdate)});
    assert(await page.locator('#install-update').isHidden());
    assert((await page.locator('#update-message').textContent()).includes('Signature'));
    await page.locator('#close-updates').click();
    assert.deepEqual(errors,[]);
    console.log('UI maquettes et Studio : 6 vues, CSP strict, navigation, IPC, permissions, réglages, presse-papiers, aperçu, restauration, filtres, états vides/erreurs et largeurs 900/1180/1440 validés.');
    console.log('Captures : target/ui-design/');
  } finally {await browser.close();server.close();}
})().catch(e=>{console.error(e);server.close();process.exitCode=1;});

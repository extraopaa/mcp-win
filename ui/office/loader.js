import init,{start} from './runtime/mcp_win_office.js';
const kind=new URLSearchParams(location.search).get('kind');
try{if(!['word','grid','deck','pdf','design'].includes(kind))throw new Error('Éditeur inconnu');await init();await start(kind);document.getElementById('office-loading').remove();}catch(e){document.getElementById('office-loading').textContent='L’éditeur ne peut pas démarrer.';parent.postMessage(JSON.stringify({type:'failed',error:String(e)}),location.origin);}

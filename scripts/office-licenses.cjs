const fs=require('node:fs'),path=require('node:path'),cp=require('node:child_process');
const root=path.resolve(__dirname,'..'),dest=path.join(root,'third-party-licenses/office');
const cargo=path.join(process.env.USERPROFILE,'.cargo/bin/cargo.exe');
const metadata=JSON.parse(cp.execFileSync(cargo,['metadata','--manifest-path',path.join(root,'tools/office-web/Cargo.toml'),'--format-version','1','--locked'],{maxBuffer:32*1024*1024,encoding:'utf8'}));
const lines=['# Moteurs bureautiques et licences','','MCP Win adapte les interfaces de WordCraft, GridCraft, DeckCraft, PdfCraft et DesignCraft. Le code de ces moteurs est réutilisé sous leur licence MIT. Les droits des auteurs sont conservés. Les marques ArtCraft sont remplacées par l’identité MCP Win. Aucun modèle OCR ocrs CC BY-SA n’est distribué dans cette intégration.','','| Composant | Version | Licence déclarée | Source |','| --- | --- | --- | --- |'];
fs.mkdirSync(dest,{recursive:true});
function copyNotices(dir,to,depth=0){if(depth>10)return;for(const e of fs.readdirSync(dir,{withFileTypes:true})){if(['target','.git','node_modules','plan'].includes(e.name))continue;const file=path.join(dir,e.name),output=path.join(to,e.name);if(e.isDirectory()){copyNotices(file,output,depth+1);}else if(/LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT|OFL|UFL|^ATTRIBUTION\.(md|toml)$/i.test(e.name)){fs.mkdirSync(to,{recursive:true});fs.copyFileSync(file,output);}}}
for(const p of metadata.packages.sort((a,b)=>a.name.localeCompare(b.name))){if(p.name==='mcp-win-office')continue;if(!p.license&&!p.license_file)throw Error('Licence absente: '+p.name);lines.push(`| ${p.name} | ${p.version} | ${p.license||'voir fichier'} | ${p.repository||p.source||'source locale'} |`);copyNotices(path.dirname(p.manifest_path),path.join(dest,p.name+'-'+p.version));}
const sources=JSON.parse(fs.readFileSync(path.join(root,'tools/office-web/sources.json'),'utf8'));
for(const[name,rev]of Object.entries(sources)){copyNotices(path.join(root,'vendor/artcraft',name),path.join(dest,'sources',name));lines.push(`\n${name} : https://github.com/storytold/${name}/tree/${rev}`);}
fs.writeFileSync(path.join(root,'docs/OFFICE-LICENSES.md'),lines.join('\n')+'\n');
console.log('Notices bureautiques générées pour '+metadata.packages.length+' composants.');

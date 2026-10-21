// Reproducible integration changes to the pinned, locally fetched upstream sources.
// Copyrights and licence notices are preserved; ArtCraft trademarks are not redistributed.
const fs=require('node:fs'),path=require('node:path');
const root=path.resolve(__dirname,'..');
const sources=JSON.parse(fs.readFileSync(path.join(root,'tools/office-web/sources.json'),'utf8'));
const labels={wordcraft:'Documents',gridcraft:'Tableur',deckcraft:'Présentation',pdfcraft:'PDF',designcraft:'Composition'};
for(const name of Object.keys(sources)){
 if(name==='deckcraft'){
  const file=path.join(root,'vendor/artcraft/deckcraft/crates/engine/src/recovery.rs');
  let s=fs.readFileSync(file,'utf8');
  if(!s.includes('// MCP Win: browser sessions do not have an OS process id.')){
   s=s.replace('        h.write_u32(std::process::id());','        // MCP Win: browser sessions do not have an OS process id.\n        #[cfg(not(target_arch = "wasm32"))]\n        h.write_u32(std::process::id());\n        #[cfg(target_arch = "wasm32")]\n        h.write_u64(js_sys::Math::random().to_bits());');
   fs.writeFileSync(file,s);
  }
 }
 if(name!=='pdfcraft'&&name!=='gridcraft'){
  const manifest=path.join(root,'vendor/artcraft',name,'crates/ui-egui/Cargo.toml');
  let toml=fs.readFileSync(manifest,'utf8');
  if(!toml.includes('dependencies.mcp-web-sys]')){toml+='\n[target.\'cfg(target_arch = "wasm32")\'.dependencies.mcp-web-sys]\npackage = "web-sys"\nversion = "0.3"\nfeatures = ["Window", "Location"]\n';fs.writeFileSync(manifest,toml);}
 }
 const dir=path.join(root,'vendor/artcraft',name,'crates/ui-egui/src');
 function walk(d){for(const e of fs.readdirSync(d,{withFileTypes:true})){const p=path.join(d,e.name);if(e.isDirectory())walk(p);else if(e.name.endsWith('.rs')){
  let s=fs.readFileSync(p,'utf8'),old=s;
  const title=name[0].toUpperCase()+name.slice(1).replace('craft','Craft');
  if(e.name==='lib.rs'&&name!=='pdfcraft'&&!s.includes('// MCP Win owns document navigation.')){
   s=s.replace(/(pub fn run\(&mut self, id: &str, params: (?:Value|Json)\) -> Result<(?:Value|Json), String> \{\r?\n)/,`$1        // MCP Win owns document navigation. Saves and close confirmation stay with the host.\n        #[cfg(target_arch = "wasm32")]\n        if matches!(id, "file.new" | "file.open" | "file.close" | "file.quit" | "app.quit" | "ui.quit") {\n            let action = if id == "file.new" { "new" } else if id == "file.open" { "open" } else { "close" };\n            if let Some(w) = mcp_web_sys::window() {\n                if let (Ok(Some(p)), Ok(o)) = (w.parent(), w.location().origin()) {\n                    let message = serde_json::json!({"type":"navigation","action":action});\n                    let _ = p.post_message(&message.to_string().into(), &o);\n                }\n            }\n            return Ok(serde_json::Value::Null);\n        }\n`);
  }
  if(name!=='pdfcraft')s=s.replaceAll('eframe::web_sys::window()', 'mcp_web_sys::window()');
  if(name==='gridcraft')s=s.replaceAll('mcp_web_sys::window()', 'web_sys::window()');
  s=s.replaceAll('"'+title+'"','"MCP Win · '+labels[name]+'"');
  s=s.replaceAll('"'+title+' — ', '"MCP Win · '+labels[name]+' — ');
  if(e.name==='chrome.rs'&&name==='designcraft')s=s.replace(/if crate::about::discord_button\(ui, "Discord", vec2\(78\.0, 22\.0\)\) \{\s*let _ = app\.run\("help\.discord", json!\(\{\}\)\);\s*\}/,'');
  if(e.name==='chrome.rs'&&name==='pdfcraft')s=s.replace(/if widgets::ghost_button_sized\(ui, "messages-square", "Discord", tab_h\)\.on_hover_text\(pdfcraft_engine::links::DISCORD\)\.clicked\(\) \{\s*app\.execute\("help\.discord"\);\s*\}/,'');
  if(e.name==='theme.rs')s=s.replace(/(accent:\s*Color32::from_rgb\()[^)]+(\))/g,'$10x71, 0x56, 0xEC$2').replace('0x3B, 0x5B, 0xDB','0x71, 0x56, 0xEC').replace('0x2B, 0x47, 0xB5','0x59, 0x40, 0xC0').replace(/accent: hex\(0x[0-9a-f]+\)/g,'accent: hex(0x7156ec)');
  if(e.name==='theme.rs'&&name==='gridcraft')s=s.replaceAll('0x10, 0x7C, 0x41','0x71, 0x56, 0xEC').replaceAll('0xC8, 0xE6, 0xD3','0xEB, 0xE6, 0xFB').replaceAll('0x23, 0x45, 0x31','0x35, 0x2C, 0x53');
  if(name==='wordcraft'&&e.name==='chrome.rs')s=s.replace(/\/\/ Account \/ community\.[\s\S]*?(?=if app\.ui\.show_discord)/,'// MCP Win owns the application header; no account badge.\n                    ');
  if(name==='pdfcraft'&&e.name==='commands.rs')for(const id of ['ocr.recognize','ocr.recognize_batch']){
   const oldBranch=id==='ocr.recognize'?`"${id}" => self.dialog = Some(Dialog::RecognizeText),`:`"${id}" => self.ocr_files_dialog(),`;
   const native=id==='ocr.recognize'?'self.dialog = Some(Dialog::RecognizeText);':'self.ocr_files_dialog();';
   s=s.replace(oldBranch,`"${id}" => { #[cfg(target_arch="wasm32")] {if let Some(w)=eframe::web_sys::window(){if let (Ok(Some(p)),Ok(o))=(w.parent(),w.location().origin()){let _=p.post_message(&"{\\\"type\\\":\\\"ocr\\\"}".into(),&o);}}} #[cfg(not(target_arch="wasm32"))] {${native}} },`);
  }
  if(name==='pdfcraft'&&e.name==='editing.rs'){
   // Native persistence must acknowledge success before the editor becomes clean.
   if(!s.includes('if !to_host { let _ = self.session.mark_saved'))s=s.replace('let _ = self.session.mark_saved(id, bytes, None);','if !to_host { let _ = self.session.mark_saved(id, bytes, None); }');
   for(const signature of ['pub fn request_close_tab(&mut self, index: usize) {','pub fn close_all(&mut self) {']){
    const guard='// MCP Win owns PDF close confirmation.';
    const at=s.indexOf(signature);
    if(at>=0&&!s.slice(at,at+signature.length+150).includes(guard))s=s.replace(signature,signature+'\n        '+guard+'\n        #[cfg(target_arch="wasm32")] { if let Some(w)=eframe::web_sys::window(){if let (Ok(Some(p)),Ok(o))=(w.parent(),w.location().origin()){let _=p.post_message(&"{\\\"type\\\":\\\"navigation\\\",\\\"action\\\":\\\"close\\\"}".into(),&o);}} return; }\n');
   }
  }
  if(name==='pdfcraft'&&e.name==='widgets.rs')s=s.replace(/include_bytes!\("\.\.\/\.\.\/\.\.\/docs\/brand\/[^\"]+"\)/g,'include_bytes!("mcp-win-logo.png")').replaceAll('artcraft-logo-white.svg','mcp-win-logo-white.png').replaceAll('artcraft-logo.svg','mcp-win-logo.png').replaceAll('artcraft-mark.svg','mcp-win-mark.png');
  if(s!==old){if(!s.startsWith('// MCP Win integration'))s='// MCP Win integration: adapted branding and purple interface tokens; original attribution retained.\n'+s;fs.writeFileSync(p,s);}
 }}}
 walk(dir);
 if(name==='pdfcraft')fs.copyFileSync(path.join(root,'assets/brand/icon-original.png'),path.join(dir,'mcp-win-logo.png'));
}
console.log('Office: identité MCP Win appliquée, mentions des auteurs conservées.');

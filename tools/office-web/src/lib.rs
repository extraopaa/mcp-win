//! MCP Win's local office host. Upstream engines retain their own MIT/Apache notices.
#![forbid(unsafe_code)]
use base64::Engine as _;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};

const MAX_BYTES: usize = 100 * 1024 * 1024;
#[derive(Default)]
struct Pending {
    messages: Vec<(Value, Vec<u8>)>,
    saves: Vec<(u64, String, Vec<u8>, bool)>,
    next: u64,
    save_as: bool,
}
#[derive(Clone)]
struct Bridge(Rc<RefCell<Pending>>);
impl Bridge {
    fn post(&self, value: Value) {
        let Some(w) = web_sys::window() else { return };
        let Ok(Some(parent)) = w.parent() else { return };
        let Ok(origin) = w.location().origin() else {
            return;
        };
        let _ = parent.post_message(&JsValue::from_str(&value.to_string()), &origin);
    }
    fn save(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > MAX_BYTES {
            return Err("Le fichier depasse 100 Mio".into());
        }
        let mut q = self.0.borrow_mut();
        if q.saves.len() >= 4 {
            return Err("Un enregistrement est deja en cours".into());
        }
        q.next += 1;
        let id = q.next;
        let save_as = std::mem::take(&mut q.save_as);
        q.saves.push((id, name.into(), bytes.to_vec(), save_as));
        Ok(())
    }
    fn listen(&self, ctx: egui::Context) -> Result<(), JsValue> {
        let w = web_sys::window().ok_or_else(|| JsValue::from_str("Window unavailable"))?;
        let parent = w
            .parent()?
            .ok_or_else(|| JsValue::from_str("Parent unavailable"))?;
        let origin = w.location().origin()?;
        let queue = self.0.clone();
        let on_message =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                if e.origin() != origin
                    || !e.source().is_some_and(|s| js_sys::Object::is(&s, &parent))
                {
                    return;
                }
                let data = e.data();
                let Ok(text) = js_sys::JSON::stringify(&data) else {
                    return;
                };
                let Some(text) = text.as_string().filter(|v| v.len() <= 1_000_000) else {
                    return;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    return;
                };
                if value["channel"] != "mcp-win-office" {
                    return;
                }
                let bytes = js_sys::Reflect::get(&data, &"bytes".into())
                    .ok()
                    .filter(|v| !v.is_undefined())
                    .map(|v| js_sys::Uint8Array::new(&v));
                if bytes
                    .as_ref()
                    .is_some_and(|v| v.length() as usize > MAX_BYTES)
                {
                    return;
                }
                let mut q = queue.borrow_mut();
                if q.messages.len() >= 32 {
                    return;
                }
                q.messages
                    .push((value, bytes.map(|v| v.to_vec()).unwrap_or_default()));
                ctx.request_repaint();
            });
        w.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref())?;
        on_message.forget();
        Ok(())
    }
}

enum Editor {
    Word(Box<wordcraft_ui_egui::WordApp>),
    Grid(Box<gridcraft_ui_egui::SheetApp>),
    Deck(Box<deckcraft_ui_egui::SlideApp>),
    Design(Box<designcraft_ui_egui::DesignApp>),
    Pdf(Box<pdfcraft_ui_egui::PdfCraftApp>),
}
impl Editor {
    fn new(kind: &str, bridge: &Bridge) -> Result<Self, String> {
        let pick = bridge.clone();
        let save = bridge.clone();
        let save_as = bridge.clone();
        Ok(match kind {
            "word" => {
                let services = wordcraft_ui_egui::Services {
                    open_async: Some(Box::new(move |purpose| {
                        pick.post(json!({"type":"pick","purpose":purpose}))
                    })),
                    pick_save: Some(Box::new(move |name| {
                        save_as.0.borrow_mut().save_as = true;
                        Some(name.into())
                    })),
                    download: Some(Box::new(move |name, bytes| save.save(name, bytes))),
                    inbox: Some(Default::default()),
                    ..Default::default()
                };
                let mut app = wordcraft_ui_egui::WordApp::new(
                    wordcraft_engine::Session::new(wordcraft_engine::blank_document(
                        &Default::default(),
                    )),
                    services,
                );
                app.ui.language = "fr".into();
                app.ui.show_discord = false;
                app.autosave = false;
                Self::Word(Box::new(app))
            }
            "grid" => {
                let services = gridcraft_ui_egui::Services {
                    pick_save: Some(Box::new(move |name| {
                        save_as.0.borrow_mut().save_as = true;
                        Some(name.into())
                    })),
                    open_async: Some(Box::new(move || {
                        pick.post(json!({"type":"pick","purpose":"document"}))
                    })),
                    download: Some(Box::new(move |name, bytes| {
                        let _ = save.save(name, bytes);
                    })),
                    ..Default::default()
                };
                let mut session = gridcraft_engine::Session::new();
                session.new_workbook();
                let mut app = gridcraft_ui_egui::SheetApp::new(session, services);
                app.ui.language = gridcraft_ui_egui::i18n::Language::Fr;
                app.chrome.title_bar = false;
                Self::Grid(Box::new(app))
            }
            "deck" => {
                let services = deckcraft_ui_egui::Services {
                    pick_save: Some(Box::new(move |name| {
                        save_as.0.borrow_mut().save_as = true;
                        Some(name.into())
                    })),
                    open_async: Some(Box::new(move |purpose, request| {
                        pick.post(json!({"type":"pick","purpose":purpose,"request":request}))
                    })),
                    download: Some(Box::new(move |name, bytes| {
                        let _ = save.save(name, bytes);
                    })),
                    inbox: Some(Default::default()),
                    ..Default::default()
                };
                Self::Deck(Box::new(deckcraft_ui_egui::SlideApp::new(
                    deckcraft_engine::Session::with_new(),
                    services,
                )))
            }
            "design" => {
                let services = designcraft_ui_egui::Services {
                    pick_save: Some(Box::new(move |name| {
                        save_as.0.borrow_mut().save_as = true;
                        Some(name.into())
                    })),
                    open_async: Some(Box::new(move |request| {
                        pick.post(json!({"type":"pick","purpose":request.purpose,"target":request.target}))
                    })),
                    download: Some(Box::new(move |name, bytes| save.save(name, bytes))),
                    inbox: Some(Default::default()),
                    ..Default::default()
                };
                let mut session = designcraft_engine::Session::new();
                session
                    .execute("file.new", &json!({}))
                    .map_err(|e| e.to_string())?;
                let mut app = designcraft_ui_egui::DesignApp::new(session, services);
                app.ui.language = "fr".into();
                app.ui.brightness = designcraft_ui_egui::theme::Brightness::Light;
                Self::Design(Box::new(app))
            }
            "pdf" => {
                let mut app = pdfcraft_ui_egui::PdfCraftApp::new();
                app.language = "fr".into();
                app.host_save = Some(Box::new(move |name, bytes, force| {
                    save.0.borrow_mut().save_as = force;
                    save.save(name, bytes)
                }));
                let blank = app
                    .session
                    .create_blank(595.28, 841.89, 1)
                    .map_err(|e| e.to_string())?;
                app.open_bytes("Document.pdf", None, (*blank).clone())?;
                Self::Pdf(Box::new(app))
            }
            _ => return Err("Editeur inconnu".into()),
        })
    }
    fn open(&mut self, name: &str, save_name: &str, bytes: Vec<u8>) -> Result<Value, String> {
        match self {
            Self::Word(app) => {
                let doc = wordcraft_engine::io::open_bytes(name, &bytes)?;
                app.session = wordcraft_engine::Session::new(doc);
                app.session.path = Some(save_name.into());
                app.ui.start_screen = false;
            }
            Self::Grid(app) => {
                let (wb, warnings) =
                    gridcraft_engine::io::open_bytes(name, &bytes).map_err(|e| e.to_string())?;
                app.session = gridcraft_engine::Session::new();
                app.session.add_document(gridcraft_engine::DocState::new(
                    wb,
                    Some(save_name.into()),
                    save_name.into(),
                ));
                return Ok(json!({"warnings": warnings}));
            }
            Self::Deck(app) => {
                let doc = deckcraft_engine::cmd::file::open_presentation(name, &bytes)
                    .map_err(|e| e.to_string())?;
                app.session = deckcraft_engine::Session::new();
                deckcraft_engine::cmd::file::add_opened(
                    &mut app.session,
                    save_name,
                    Some(save_name.into()),
                    doc,
                );
            }
            Self::Design(app) => {
                app.session.execute("file.openBytes", &json!({"name":name,"base64":base64::engine::general_purpose::STANDARD.encode(bytes)})).map_err(|e| e.to_string())?;
                if let Some(d) = app.session.active_mut() {
                    d.path = Some(save_name.into());
                }
            }
            Self::Pdf(app) => {
                let save = app.host_save.take();
                let mut fresh = pdfcraft_ui_egui::PdfCraftApp::new();
                fresh.language = "fr".into();
                fresh.host_save = save;
                fresh.open_bytes(name, None, bytes)?;
                **app = fresh;
            }
        }
        Ok(json!({}))
    }
    fn command(&mut self, id: &str, params: Value) -> Result<Value, String> {
        // Never allow an engine command to resolve arbitrary paths, network resources or OS actions.
        if id.starts_with("file.")
            || id.starts_with("app.")
            || id.starts_with("ui.")
            || id.contains("print")
            || id.contains("export")
            || id.contains("open")
            || id.contains("link.follow")
        {
            return Err("Utilisez les commandes de fichiers de MCP Win".into());
        }
        match self {
            Self::Word(a) => a.session.run(id, &params).map_err(|e| e.to_string()),
            Self::Grid(a) => a.session.execute(id, params).map_err(|e| e.to_string()),
            Self::Deck(a) => a.session.execute(id, &params).map_err(|e| e.to_string()),
            Self::Design(a) => a.session.execute(id, &params).map_err(|e| e.to_string()),
            Self::Pdf(_) => Err("Commande PDF non disponible par ce canal".into()),
        }
    }
    fn catalog(&self) -> Value {
        match self {
            Self::Word(a) => a.session.registry.describe(),
            Self::Grid(a) => serde_json::to_value(a.session.commands()).unwrap_or(Value::Null),
            Self::Deck(a) => serde_json::to_value(a.session.commands()).unwrap_or(Value::Null),
            Self::Design(a) => serde_json::to_value(a.session.commands()).unwrap_or(Value::Null),
            Self::Pdf(_) => json!([]),
        }
    }
    fn asset(&mut self, m: &Value, bytes: Vec<u8>) -> Result<Value, String> {
        let name = m["name"].as_str().unwrap_or("image.png").to_string();
        match self {
            Self::Word(a) => {
                a.session
                    .run(
                        "insert.picture",
                        &json!({"data":base64::engine::general_purpose::STANDARD.encode(bytes)}),
                    )
                    .map_err(|e| e.to_string())?;
            }
            Self::Deck(a) => {
                let id = m["request"]
                    .as_u64()
                    .unwrap_or_else(|| a.begin_external_file("picture"));
                if let Some(inbox) = &a.services.inbox {
                    inbox.lock().map_err(|_| "Import indisponible")?.push(
                        deckcraft_ui_egui::InboxEvent::File(deckcraft_ui_egui::InboxFile {
                            id,
                            name,
                            bytes,
                        }),
                    );
                }
            }
            Self::Design(a) => {
                if let Some(inbox) = &a.services.inbox {
                    inbox.lock().map_err(|_| "Import indisponible")?.push(
                        designcraft_ui_egui::ImportedFile {
                            request: designcraft_ui_egui::ImportRequest {
                                purpose: m["purpose"].as_str().unwrap_or("place").into(),
                                target: m["target"].as_u64(),
                            },
                            name,
                            bytes,
                        },
                    );
                }
            }
            _ => return Err("Import d’image indisponible dans cet editeur".into()),
        }
        Ok(json!({}))
    }
    fn mark_saved(&mut self, m: &Value) -> Value {
        let state = self.state();
        if state["document"] != m["document"] || state["revision"] != m["revision"] {
            return json!({"changed":true});
        }
        match self {
            Self::Word(a) => {
                a.session.dirty = false;
                if let Some(name) = m["name"].as_str() {
                    a.session.path = Some(name.into())
                }
            }
            Self::Grid(a) => {
                if let Some(d) = a.session.active_mut() {
                    d.saved = d.wb.clone();
                    if let Some(name) = m["name"].as_str() {
                        d.path = Some(name.into());
                        d.title = name.into()
                    }
                }
            }
            Self::Deck(a) => {
                if let Some(d) = a.session.active_mut() {
                    d.saved_doc = d.doc.clone();
                    if let Some(name) = m["name"].as_str() {
                        d.path = Some(name.into());
                        d.title = name.into()
                    }
                }
            }
            Self::Design(a) => {
                if let Some(d) = a.session.active_mut() {
                    d.saved_doc = d.doc.clone();
                    d.saved_revision = d.revision;
                    if let Some(name) = m["name"].as_str() {
                        d.path = Some(name.into())
                    }
                }
            }
            Self::Pdf(a) => {
                if let Some((_, id)) = a.active_ids() {
                    if let Ok(bytes) = a.session.save_bytes(id) {
                        let _ = a.session.mark_saved(id, bytes, None);
                    }
                }
            }
        }
        json!({"saved":true})
    }
    fn save(&mut self, name: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::Word(a) => wordcraft_engine::io::save_bytes(name, &a.session.doc),
            Self::Grid(a) => gridcraft_engine::io::save_bytes(
                &a.session.doc().map_err(|e| e.to_string())?.wb,
                name,
            )
            .map_err(|e| e.to_string()),
            Self::Deck(a) => {
                let doc = &a.session.doc().map_err(|e| e.to_string())?.doc;
                let ext = name.rsplit('.').next().unwrap_or("pptx");
                if ext == "pdf" {
                    deckcraft_engine::cmd::file::pdf_bytes(doc, &json!({}))
                        .map_err(|e| e.to_string())
                } else {
                    deckcraft_engine::cmd::file::save_bytes(doc, ext).map_err(|e| e.to_string())
                }
            }
            Self::Design(a) => {
                designcraft_format::save(&a.session.doc().map_err(|e| e.to_string())?.doc)
                    .map_err(|e| e.to_string())
            }
            Self::Pdf(a) => {
                a.run_command("file.save");
                Ok(Vec::new())
            }
        }
    }
    fn state(&self) -> Value {
        match self {
            Self::Word(a) => json!({"dirty":a.session.dirty,"revision":a.session.rev(),"document":a.session.document_id(),"undo":a.session.can_undo(),"redo":a.session.can_redo()}),
            Self::Grid(a) => a.session.active().map(|d| json!({"dirty":d.is_dirty(),"revision":d.revision,"document":d.uid,"sheets":d.wb.sheets.iter().map(|s| &s.name).collect::<Vec<_>>() })).unwrap_or(json!({"dirty":false})),
            Self::Deck(a) => a.session.active().map(|d| json!({"dirty":d.is_dirty(),"revision":d.revision,"document":d.uid,"slides":d.doc.slides.len()})).unwrap_or(json!({"dirty":false})),
            Self::Design(a) => a.session.active().map(|d| json!({"dirty":d.is_dirty(),"revision":d.revision,"document":d.uid})).unwrap_or(json!({"dirty":false})),
            Self::Pdf(a) => a.active_ids().and_then(|(_,id)|a.session.get(id).map(|d|json!({"dirty":a.session.docs().iter().any(|doc|doc.dirty),"revision":d.edit_generation(),"document":id.0,"pending_documents":a.session.docs().iter().filter(|doc|doc.dirty).map(|doc|doc.id.0).collect::<Vec<_>>()}))).unwrap_or(json!({"dirty":false})),
        }
    }
}
struct Host {
    editor: Editor,
    bridge: Bridge,
    state: Value,
}
impl eframe::App for Host {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let messages = std::mem::take(&mut self.bridge.0.borrow_mut().messages);
        for (m, bytes) in messages {
            let result = match m["action"].as_str().unwrap_or("") {
                "open" => self.editor.open(
                    m["name"].as_str().unwrap_or("document.docx"),
                    m["save_name"]
                        .as_str()
                        .or_else(|| m["name"].as_str())
                        .unwrap_or("document.docx"),
                    bytes,
                ),
                "command" => self
                    .editor
                    .command(m["command"].as_str().unwrap_or(""), m["params"].clone()),
                "state" => Ok(self.editor.state()),
                "catalog" => Ok(self.editor.catalog()),
                "saved" => Ok(self.editor.mark_saved(&m)),
                "asset" => self.editor.asset(&m, bytes),
                "save" => self
                    .editor
                    .save(m["name"].as_str().unwrap_or("document.docx"))
                    .and_then(|b| {
                        if !b.is_empty() {
                            self.bridge
                                .save(m["name"].as_str().unwrap_or("document.docx"), &b)?;
                        }
                        Ok(json!({}))
                    }),
                _ => Err("Action inconnue".into()),
            };
            self.bridge.post(match result {
                Ok(value) => json!({"type":"reply","id":m["id"],"result":value}),
                Err(e) => json!({"type":"reply","id":m["id"],"error":e}),
            });
        }
        match &mut self.editor {
            Editor::Word(a) => a.logic(ctx),
            Editor::Grid(a) => a.logic(ctx),
            Editor::Deck(a) => a.logic(ctx),
            Editor::Design(a) => a.logic(ctx),
            Editor::Pdf(a) => a.logic(ctx, frame),
        }
        let state = self.editor.state();
        if state != self.state {
            self.state = state.clone();
            self.bridge.post(json!({"type":"state","state":state}));
        }
        let saves = std::mem::take(&mut self.bridge.0.borrow_mut().saves);
        for (ticket, name, bytes, save_as) in saves {
            if let Some(w) = web_sys::window() {
                if let (Ok(Some(parent)), Ok(origin)) = (w.parent(), w.location().origin()) {
                    let m = js_sys::Object::new();
                    let _ = js_sys::Reflect::set(&m, &"type".into(), &"save".into());
                    let _ = js_sys::Reflect::set(
                        &m,
                        &"ticket".into(),
                        &JsValue::from_f64(ticket as f64),
                    );
                    let _ = js_sys::Reflect::set(&m, &"name".into(), &name.into());
                    let _ =
                        js_sys::Reflect::set(&m, &"saveAs".into(), &JsValue::from_bool(save_as));
                    let data = js_sys::Uint8Array::from(bytes.as_slice()).buffer();
                    let _ = js_sys::Reflect::set(&m, &"bytes".into(), &data);
                    let _ =
                        parent.post_message_with_transfer(&m, &origin, &js_sys::Array::of1(&data));
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(150));
    }
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        match &mut self.editor {
            Editor::Word(a) => a.raw_input_hook(raw),
            Editor::Grid(a) => a.raw_input_hook(raw),
            Editor::Deck(a) => a.raw_input_hook(raw),
            Editor::Design(a) => a.raw_input_hook(raw),
            Editor::Pdf(_) => {}
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        match &mut self.editor {
            Editor::Word(a) => a.ui(ui),
            Editor::Grid(a) => a.ui(ui),
            Editor::Deck(a) => a.ui(ui),
            Editor::Design(a) => a.ui(ui),
            Editor::Pdf(a) => a.ui(ui, frame),
        }
    }
}

#[wasm_bindgen]
pub async fn start(kind: String) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let w = web_sys::window().ok_or_else(|| JsValue::from_str("No window"))?;
    let canvas = w
        .document()
        .and_then(|d| d.get_element_by_id("office-canvas"))
        .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| JsValue::from_str("No canvas"))?;
    let bridge = Bridge(Rc::default());
    let editor = Editor::new(&kind, &bridge).map_err(|e| JsValue::from_str(&e))?;
    let ready = bridge.clone();
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions {
                renderer: eframe::Renderer::Glow,
                ..Default::default()
            },
            Box::new(move |cc| {
                let _ = bridge.listen(cc.egui_ctx.clone());
                Ok(Box::new(Host {
                    editor,
                    bridge,
                    state: Value::Null,
                }))
            }),
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("{e:?}")))?;
    ready.post(json!({"type":"ready"}));
    Ok(())
}

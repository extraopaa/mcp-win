use super::{design, quality, state};
use crate::{
    advanced,
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
#[derive(Deserialize, schemars::JsonSchema)]
pub struct InspectArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
#[derive(Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Edit {
    pub id: String,
    #[serde(default)]
    pub text: String,
    pub find: Option<String>,
    pub replacement: Option<String>,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct EditArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub output_path: String,
    pub expected_sha256: String,
    pub edits: Vec<Edit>,
    pub label: Option<String>,
}
#[derive(Serialize)]
struct Element {
    id: String,
    kind: String,
    text: String,
    part: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Revision {
    pub id: String,
    pub status: String,
    pub label: String,
    pub root: PathBuf,
    pub path: String,
    pub output_path: String,
    pub source_sha256: String,
    pub staged_sha256: String,
    pub created: u64,
    pub diff: String,
    pub backup: Option<String>,
    #[serde(default)]
    pub applying: bool,
    pub model: Option<design::Args>,
    #[serde(default)]
    pub identity: Option<state::Brand>,
    #[serde(default)]
    pub assets: std::collections::BTreeMap<String, String>,
}
fn xml_elements(name: &str, xml: &str) -> Result<Vec<(String, std::ops::Range<usize>)>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let mut out = vec![];
    let mut p = 0;
    let mut t = 0;
    for n in doc.descendants().filter(|n| n.is_element()) {
        if n.tag_name().name() == "p" {
            p += 1;
            if n.descendants()
                .any(|x| x.is_element() && x.tag_name().name() == "t")
            {
                out.push((format!("{name}#paragraph-{p}"), n.range()));
            }
        }
        if n.tag_name().name() == "tbl" {
            t += 1;
            for (r, row) in n
                .children()
                .filter(|x| x.is_element() && x.tag_name().name() == "tr")
                .enumerate()
            {
                for (c, cell) in row
                    .children()
                    .filter(|x| x.is_element() && x.tag_name().name() == "tc")
                    .enumerate()
                {
                    out.push((
                        format!("{name}#table-{t}.row-{}.cell-{}", r + 1, c + 1),
                        cell.range(),
                    ));
                }
            }
        }
    }
    Ok(out)
}
fn plain(xml: &str) -> String {
    roxmltree::Document::parse(xml)
        .map(|d| {
            d.descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "t")
                .filter_map(|n| n.text())
                .collect()
        })
        .unwrap_or_default()
}
fn model(folders: &[SharedFolder], folder: &str, path: &str, hash: &str) -> Option<design::Args> {
    let full = library::resolve(folders, folder, path).ok()?;
    let file = state::root()
        .ok()?
        .join("models")
        .join(format!("{}.json", state::scope(&full)));
    let data: Value = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    if data["sha256"].as_str() != Some(hash) {
        return None;
    }
    let mut args: design::Args = serde_json::from_value(data["args"].clone()).ok()?;
    args.internal_brand = serde_json::from_value(data["identity"].clone()).ok();
    args.internal_asset_hashes = serde_json::from_value(data["assets"].clone()).unwrap_or_default();
    Some(args)
}
pub fn inspect(folders: &[SharedFolder], args: InspectArgs) -> Result<Value, String> {
    let path = advanced::input(folders, &args.folder, &args.path)?;
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let hash = writing::hash(&bytes);
    let mut elements = vec![];
    if let Some(model) = model(folders, &args.folder, &args.path, &hash) {
        for b in design::blocks(&model)? {
            elements.push(Element {
                id: b.id.clone(),
                kind: b.kind.clone(),
                text: if b.kind == "list" {
                    b.items.join("\n")
                } else {
                    b.text.clone()
                },
                part: None,
            });
            if b.kind == "table" {
                for (r, row) in b.rows.iter().enumerate() {
                    for (c, text) in row.iter().enumerate() {
                        elements.push(Element {
                            id: format!("{}.row-{}.cell-{}", b.id, r + 1, c + 1),
                            kind: "cell".into(),
                            text: text.clone(),
                            part: None,
                        })
                    }
                }
            }
        }
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
    {
        crate::office::archive(&bytes)?;
        let book =
            umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(bytes.clone()), true)
                .map_err(|e| e.to_string())?;
        for sheet in book.sheet_collection() {
            let mut cells: Vec<_> = sheet.collection_to_hashmap().values().collect();
            cells.sort_by_key(|c| c.coordinate().get_coordinate());
            for cell in cells {
                if elements.len() >= 200_000 {
                    return Err("Inspection limitée à 200 000 cellules".into());
                }
                elements.push(Element {
                    id: format!("{}!{}", sheet.name(), cell.coordinate().get_coordinate()),
                    kind: format!("cell:{}", cell.data_type()),
                    text: if cell.formula().is_empty() {
                        cell.value().to_string()
                    } else {
                        format!("={}", cell.formula())
                    },
                    part: Some(sheet.name().into()),
                })
            }
        }
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("docx") || e.eq_ignore_ascii_case("pptx"))
    {
        for (name, data) in design::unpack(&bytes)? {
            if (name.starts_with("word/") || name.starts_with("ppt/slides/"))
                && name.ends_with(".xml")
                && !name.contains("_rels")
            {
                let xml = String::from_utf8(data).map_err(|e| e.to_string())?;
                for (id, range) in xml_elements(&name, &xml)? {
                    elements.push(Element {
                        id,
                        kind: "text_element".into(),
                        text: plain_fragment(&xml[range], &xml),
                        part: Some(name.clone()),
                    })
                }
            }
        }
    } else if let Ok(text) = std::str::from_utf8(&bytes) {
        for (i, line) in text.lines().enumerate() {
            elements.push(Element {
                id: format!("line-{}", i + 1),
                kind: "line".into(),
                text: line.into(),
                part: None,
            })
        }
    } else {
        return Err("Éléments disponibles : documents générés par MCP Win, DOCX/PPTX ou texte UTF-8. PDF externes : outils de pages et formulaires.".into());
    }
    let total = elements.len();
    let offset = args.offset.unwrap_or(0);
    let limit = args.limit.unwrap_or(50).clamp(1, 200);
    let items: Vec<_> = elements
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|mut e| {
            let truncated = e.text.chars().count() > 1500;
            if truncated {
                e.text = e.text.chars().take(1500).collect()
            }
            let mut value = serde_json::to_value(e).unwrap();
            value["text_truncated"] = json!(truncated);
            value
        })
        .collect();
    let end = offset + items.len();
    Ok(
        json!({"sha256":hash,"elements":items,"total":total,"next_offset":(end<total).then_some(end),"hint":"Envoyer les id à edit_document_elements. L’empreinte est obligatoire ; une proposition révisable est créée avant application locale."}),
    )
}
fn plain_fragment(fragment: &str, full: &str) -> String {
    let Ok(doc) = roxmltree::Document::parse(full) else {
        return String::new();
    };
    let namespaces = doc
        .root_element()
        .namespaces()
        .map(|n| {
            format!(
                "xmlns{}=\"{}\"",
                n.name().map(|s| format!(":{s}")).unwrap_or_default(),
                design::escape(n.uri())
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    plain(&format!("<root {namespaces}>{fragment}</root>"))
}

fn replace_element(
    xml: &str,
    range: std::ops::Range<usize>,
    edit: &Edit,
) -> Result<String, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let element = doc
        .descendants()
        .find(|n| n.is_element() && n.range() == range)
        .ok_or("Élément introuvable")?;
    let nodes: Vec<_> = element
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "t")
        .collect();
    let current = nodes.iter().filter_map(|n| n.text()).collect::<String>();
    let (find, replacement) = if let Some(find) = &edit.find {
        if find.is_empty() || current.matches(find).count() != 1 {
            return Err("Le texte ciblé doit apparaître exactement une fois dans l’élément".into());
        }
        (
            find.as_str(),
            edit.replacement.as_deref().unwrap_or(&edit.text),
        )
    } else {
        (current.as_str(), edit.text.as_str())
    };
    if nodes.is_empty() {
        return Err("L’élément ne contient pas de texte modifiable".into());
    }
    let start = current.find(find).unwrap_or(0);
    let end = start + find.len();
    let mut at = 0;
    let mut inserted = false;
    let mut patches = vec![];
    for node in nodes {
        let old = node.text().unwrap_or("");
        let stop = at + old.len();
        if (stop > start && at < end) || (find.is_empty() && !inserted) {
            let mut new = String::new();
            if start > at {
                new.push_str(&old[..start - at])
            }
            if !inserted {
                new.push_str(replacement);
                inserted = true
            }
            if end < stop {
                new.push_str(&old[end.saturating_sub(at)..])
            }
            let tag = &xml[node.range()];
            let open = tag.find('>').ok_or("Balise texte invalide")?;
            let close = tag.rfind('<').ok_or("Balise texte invalide")?;
            if close <= open {
                return Err("Nœud texte vide non modifiable".into());
            }
            patches.push((
                node.range().start + open + 1..node.range().start + close,
                design::escape(&new),
            ));
        }
        at = stop;
    }
    if !inserted {
        return Err("Texte non trouvé".into());
    }
    let mut out = xml.to_owned();
    for (range, text) in patches.into_iter().rev() {
        out.replace_range(range, &text)
    }
    roxmltree::Document::parse(&out).map_err(|e| e.to_string())?;
    Ok(out)
}
async fn stage_model(
    folders: &[SharedFolder],
    mut args: design::Args,
) -> Result<(Vec<u8>, Value), String> {
    let original = library::select_folder(folders, &args.folder)?.id.clone();
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    for (i, b) in args.blocks.iter_mut().enumerate() {
        if b.kind == "image" {
            let src = advanced::input(folders, &original, &b.path)?;
            let name = format!("image-{i}.png");
            fs::copy(src, temp.path().join(&name)).map_err(|e| e.to_string())?;
            b.path = name;
        }
    }
    for c in &mut args.citations {
        if c.folder.is_empty() {
            c.folder = original.clone()
        }
    }
    let ext = std::path::Path::new(&args.path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    args.path = format!("proposition.{ext}");
    args.folder = format!("stage-{}", uuid::Uuid::new_v4().simple());
    let mut scope = folders.to_vec();
    scope.push(SharedFolder {
        id: args.folder.clone(),
        path: temp.path().into(),
        writable: true,
    });
    let result = design::create(&scope, args.clone()).await?;
    let bytes = fs::read(temp.path().join(&args.path)).map_err(|e| e.to_string())?;
    let key = state::scope(&temp.path().join(&args.path));
    let _ = fs::remove_file(state::root()?.join("models").join(format!("{key}.json")));
    Ok((bytes, result["quality"].clone()))
}
pub async fn propose(folders: &[SharedFolder], mut args: EditArgs) -> Result<Value, String> {
    if args.edits.is_empty()
        || args.edits.len() > 100
        || args
            .edits
            .iter()
            .map(|e| e.text.len() + e.replacement.as_ref().map_or(0, String::len))
            .sum::<usize>()
            > 200_000
    {
        return Err("1–100 modifications, 200 Ko maximum".into());
    }
    let full = advanced::input(folders, &args.folder, &args.path)?;
    let source = fs::read(&full).map_err(|e| e.to_string())?;
    if writing::hash(&source) != args.expected_sha256 {
        return Err("Source modifiée : relire les éléments".into());
    }
    let out = writing::destination(folders, &args.folder, &args.output_path)?;
    if out.exists() && out != full {
        return Err("La copie de sortie existe déjà".into());
    }
    if out == full {
        args.output_path = args.path.clone()
    }
    if full.extension() != out.extension() {
        return Err("Conserver la même extension".into());
    }
    let mut model = model(folders, &args.folder, &args.path, &args.expected_sha256);
    let mut diff_before = String::new();
    let mut diff_after = String::new();
    let (bytes, quality) = if let Some(ref mut doc) = model {
        doc.folder = library::select_folder(folders, &args.folder)?.id.clone();
        let mut blocks = design::blocks(doc)?;
        for edit in &args.edits {
            let mut found = false;
            for b in &mut blocks {
                if b.id == edit.id {
                    let old = if b.kind == "list" {
                        b.items.join("\n")
                    } else {
                        b.text.clone()
                    };
                    let new = if let Some(find) = &edit.find {
                        if find.is_empty() || old.matches(find).count() != 1 {
                            return Err("Texte à remplacer ambigu".into());
                        }
                        old.replacen(find, edit.replacement.as_deref().unwrap_or(&edit.text), 1)
                    } else {
                        edit.text.clone()
                    };
                    diff_before.push_str(&format!("{}: {}\n", b.id, old));
                    diff_after.push_str(&format!("{}: {}\n", b.id, new));
                    if b.kind == "list" {
                        b.items = new.lines().map(str::to_owned).collect()
                    } else {
                        b.text = new
                    }
                    found = true;
                    break;
                }
                if let Some(suffix) = edit.id.strip_prefix(&format!("{}.row-", b.id)) {
                    let (r, c) = suffix.split_once(".cell-").ok_or("Cellule invalide")?;
                    let row = r
                        .parse::<usize>()
                        .map_err(|_| "Ligne invalide")?
                        .checked_sub(1)
                        .ok_or("Ligne >=1")?;
                    let col = c
                        .parse::<usize>()
                        .map_err(|_| "Colonne invalide")?
                        .checked_sub(1)
                        .ok_or("Colonne >=1")?;
                    let cell = b
                        .rows
                        .get_mut(row)
                        .and_then(|r| r.get_mut(col))
                        .ok_or("Cellule inexistante")?;
                    diff_before.push_str(&format!("{}: {}\n", edit.id, cell));
                    *cell = edit.text.clone();
                    diff_after.push_str(&format!("{}: {}\n", edit.id, cell));
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(format!("Élément inconnu : {}", edit.id));
            }
        }
        doc.blocks = blocks;
        doc.sections.clear();
        doc.path = args.output_path.clone();
        let (staged, quality) = stage_model(folders, doc.clone()).await?;
        (staged, quality)
    } else if full
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
    {
        crate::office::archive(&source)?;
        let mut book =
            umya_spreadsheet::reader::xlsx::read_reader(std::io::Cursor::new(source.clone()), true)
                .map_err(|e| e.to_string())?;
        for edit in &args.edits {
            let (sheet, address) = edit
                .id
                .rsplit_once('!')
                .ok_or("Cellule attendue : Feuille!A1")?;
            if !crate::generate::valid_cell(address) {
                return Err("Adresse de cellule invalide".into());
            }
            let cell = book
                .sheet_by_name_mut(sheet)
                .map_err(|e| e.to_string())?
                .cell_mut(address);
            let kind = cell.data_type().to_owned();
            let old = if cell.formula().is_empty() {
                cell.value().to_string()
            } else {
                format!("={}", cell.formula())
            };
            let new = if let Some(find) = &edit.find {
                if find.is_empty() || old.matches(find).count() != 1 {
                    return Err("Texte de cellule ambigu".into());
                }
                old.replacen(find, edit.replacement.as_deref().unwrap_or(&edit.text), 1)
            } else {
                edit.text.clone()
            };
            diff_before.push_str(&format!("{}: {}\n", edit.id, old));
            diff_after.push_str(&format!("{}: {}\n", edit.id, new));
            if !cell.formula().is_empty() && new.starts_with('=') {
                cell.set_formula(new.trim_start_matches('='));
            } else if kind == "n" {
                let n = new
                    .parse::<f64>()
                    .map_err(|_| "Cellule numérique : conserver un nombre avec point décimal")?;
                if !n.is_finite() {
                    return Err("Nombre non fini".into());
                }
                cell.set_value_number(n);
            } else if kind == "b" {
                cell.set_value_bool(
                    new.parse::<bool>()
                        .map_err(|_| "Booléen true/false requis")?,
                );
            } else {
                cell.set_value_string(new);
            }
        }
        let mut bytes = vec![];
        umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes)
            .map_err(|e| e.to_string())?;
        let q = quality::inspect_bytes(&bytes, "xlsx")?;
        (bytes, q)
    } else if full
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("docx") || e.eq_ignore_ascii_case("pptx"))
    {
        let mut parts = design::unpack(&source)?;
        let mut grouped: std::collections::BTreeMap<String, Vec<&Edit>> =
            std::collections::BTreeMap::new();
        for edit in &args.edits {
            let (part, _) = edit
                .id
                .split_once('#')
                .ok_or("Identifiant d’élément invalide")?;
            if !(part.starts_with("word/") || part.starts_with("ppt/slides/"))
                || !part.ends_with(".xml")
            {
                return Err("Partie documentaire invalide".into());
            }
            grouped.entry(part.into()).or_default().push(edit);
        }
        for (part, edits) in grouped {
            let mut xml = String::from_utf8(parts.get(&part).ok_or("Partie absente")?.clone())
                .map_err(|e| e.to_string())?;
            let available = xml_elements(&part, &xml)?;
            let mut selected = vec![];
            for edit in edits {
                let range = available
                    .iter()
                    .find(|(id, _)| id == &edit.id)
                    .map(|(_, r)| r.clone())
                    .ok_or("Élément absent")?;
                selected.push((range, edit))
            }
            selected.sort_by_key(|(r, _)| r.start);
            for pair in selected.windows(2) {
                if pair[0].0.end > pair[1].0.start {
                    return Err("Modifications chevauchantes : choisir le paragraphe ou la cellule, pas les deux".into());
                }
            }
            for (range, edit) in selected.into_iter().rev() {
                diff_before.push_str(&format!(
                    "{}: {}\n",
                    edit.id,
                    plain_fragment(&xml[range.clone()], &xml)
                ));
                xml = replace_element(&xml, range, edit)?;
                diff_after.push_str(&format!(
                    "{}: {}\n",
                    edit.id,
                    edit.replacement.as_ref().unwrap_or(&edit.text)
                ));
            }
            parts.insert(part, xml.into_bytes());
        }
        let bytes = design::pack(parts)?;
        let quality = quality::inspect_bytes(&bytes, &full.extension().unwrap().to_string_lossy())?;
        (bytes, quality)
    } else {
        let text =
            std::str::from_utf8(&source).map_err(|_| "Format non modifiable par éléments")?;
        let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let trailing = text.ends_with('\n');
        let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
        diff_before = text.to_owned();
        for edit in &args.edits {
            let i = edit
                .id
                .strip_prefix("line-")
                .ok_or("Identifiant line-N requis")?
                .parse::<usize>()
                .map_err(|_| "Ligne invalide")?
                .checked_sub(1)
                .ok_or("Ligne >=1")?;
            let line = lines.get_mut(i).ok_or("Ligne inexistante")?;
            if let Some(find) = &edit.find {
                if find.is_empty() || line.matches(find).count() != 1 {
                    return Err("Texte ambigu".into());
                }
                *line = line.replacen(find, edit.replacement.as_deref().unwrap_or(&edit.text), 1)
            } else {
                *line = edit.text.clone()
            }
        }
        let mut changed = lines.join(ending);
        if trailing {
            changed.push_str(ending)
        }
        diff_after = changed.clone();
        let ext = full.extension().unwrap_or_default().to_string_lossy();
        if ext == "json" {
            serde_json::from_str::<Value>(&changed).map_err(|e| e.to_string())?;
        }
        if ["xml", "svg"].contains(&ext.as_ref()) {
            roxmltree::Document::parse(&changed).map_err(|e| e.to_string())?;
        }
        (changed.into_bytes(), json!({"passed":true}))
    };
    if quality["passed"] == false {
        return Err(format!("Qualité insuffisante : {quality}"));
    }
    let id = uuid::Uuid::new_v4().simple().to_string();
    let root = library::select_folder(folders, &args.folder)?
        .path
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let patch = diffy::create_patch(&diff_before, &diff_after).to_string();
    let dir = state::root()?.join("revisions");
    state::reserve(&dir, bytes.len() as u64 + 6_000_000, 512 * 1024 * 1024)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let revision = Revision {
        id: id.clone(),
        status: "pending".into(),
        applying: false,
        label: args
            .label
            .unwrap_or_else(|| "Modification documentaire".into()),
        root,
        path: args.path,
        output_path: args.output_path,
        source_sha256: args.expected_sha256,
        staged_sha256: writing::hash(&bytes),
        created: state::now(),
        diff: patch.chars().take(64000).collect(),
        backup: None,
        identity: model.as_ref().and_then(|m| m.internal_brand.clone()),
        assets: model
            .as_ref()
            .map(|m| m.internal_asset_hashes.clone())
            .unwrap_or_default(),
        model,
    };
    fs::write(dir.join(format!("{id}.bin")), bytes).map_err(|e| e.to_string())?;
    state::write_json(&dir.join(format!("{id}.json")), &revision)?;
    Ok(
        json!({"revision_id":id,"status":"pending","path":revision.output_path,"diff":revision.diff,"quality":quality,"next_action":"Faire valider la proposition dans Révisions, MCP arrêté. Aucun fichier partagé n’a été modifié."}),
    )
}
fn load(id: &str) -> Result<Revision, String> {
    state::valid_id(id)?;
    let file = state::root()?.join("revisions").join(format!("{id}.json"));
    if fs::metadata(&file).map_err(|e| e.to_string())?.len() > 6_000_000 {
        return Err("Révision trop grande".into());
    }
    serde_json::from_slice(&fs::read(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
pub fn preview(folders: &[SharedFolder], id: &str) -> Result<Value, String> {
    use base64::Engine;
    let r = load(id)?;
    let folder = folders
        .iter()
        .find(|f| f.path.canonicalize().ok().as_ref() == Some(&r.root))
        .ok_or("Dossier non partagé")?;
    if !r.path.to_ascii_lowercase().ends_with(".pdf") {
        return Ok(
            json!({"images":[],"note":"Comparaison du contenu Office ; contrôler le rendu dans votre lecteur après application."}),
        );
    }
    if r.status != "pending" {
        return Ok(json!({"images":[],"note":"La proposition a déjà été traitée."}));
    }
    let original =
        fs::read(advanced::input(folders, &folder.id, &r.path)?).map_err(|e| e.to_string())?;
    if writing::hash(&original) != r.source_sha256 {
        return Err("La source a changé depuis la proposition : recréer la révision".into());
    }
    let proposed = fs::read(state::root()?.join("revisions").join(format!("{id}.bin")))
        .map_err(|e| e.to_string())?;
    if writing::hash(&proposed) != r.staged_sha256 {
        return Err("Proposition altérée".into());
    }
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    fs::write(temp.path().join("before.pdf"), original).map_err(|e| e.to_string())?;
    fs::write(temp.path().join("after.pdf"), proposed).map_err(|e| e.to_string())?;
    let scope = vec![SharedFolder {
        id: "preview".into(),
        path: temp.path().into(),
        writable: false,
    }];
    let mut images = vec![];
    for (path, label) in [("before.pdf", "Avant"), ("after.pdf", "Après")] {
        let (image, pages) = advanced::pdf_image(&scope, "", path, 1, 800)?;
        images.push(json!({"label":label,"page":1,"pages":pages,"png":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(advanced::png(&image)?))}));
        for page in 2..=pages.min(3) {
            let (image, _) = advanced::pdf_image(&scope, "", path, page, 800)?;
            images.push(json!({"label":label,"page":page,"pages":pages,"png":format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(advanced::png(&image)?))}))
        }
    }
    Ok(
        json!({"images":images,"note":"Aperçus PDF réels : trois premières pages de chaque version. Aucun fichier partagé modifié."}),
    )
}
pub fn list() -> Result<Value, String> {
    let dir = state::root()?.join("revisions");
    let mut paths = vec![];
    if dir.exists() {
        for file in fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            if file.path().extension().is_some_and(|e| e == "json") {
                paths.push((file.metadata().and_then(|m| m.modified()).ok(), file.path()))
            }
        }
    }
    paths.sort_by_key(|p| std::cmp::Reverse(p.0));
    let mut out = vec![];
    for (_, path) in paths.into_iter().take(200) {
        let id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if let Ok(v) = load(&id) {
            out.push(json!({"id":v.id,"status":v.status,"label":v.label,"path":v.path,"output_path":v.output_path,"created":v.created,"diff":v.diff,"pdf_preview":v.output_path.to_ascii_lowercase().ends_with(".pdf")}))
        }
    }
    out.sort_by_key(|v| std::cmp::Reverse(v["created"].as_u64().unwrap_or(0)));
    Ok(json!({"revisions":out}))
}

pub fn apply(folders: &[SharedFolder], id: &str, action: &str) -> Result<Value, String> {
    let _lock = state::LOCK.lock().map_err(|_| "Révisions occupées")?;
    let mut revision = load(id)?;
    let folder = folders
        .iter()
        .find(|f| f.path.canonicalize().ok().as_ref() == Some(&revision.root))
        .ok_or("Le dossier source n’est plus partagé")?;
    let result = match action {
        "reject" if revision.status == "pending" => {
            revision.status = "rejected".into();
            json!({"rejected":true})
        }
        "apply" if revision.status == "pending" => {
            let target = writing::destination(folders, &folder.id, &revision.output_path)?;
            let recovered = revision.applying
                && fs::read(&target).is_ok_and(|b| writing::hash(&b) == revision.staged_sha256);
            let source = advanced::input(folders, &folder.id, &revision.path)?;
            let source_bytes = fs::read(source).map_err(|e| e.to_string())?;
            if !recovered && writing::hash(&source_bytes) != revision.source_sha256 {
                return Err(
                    "Source modifiée depuis la proposition : créer une nouvelle révision".into(),
                );
            }
            let bytes = fs::read(state::root()?.join("revisions").join(format!("{id}.bin")))
                .map_err(|e| e.to_string())?;
            if writing::hash(&bytes) != revision.staged_sha256 {
                return Err("Proposition altérée".into());
            }
            if !recovered {
                if revision.output_path == revision.path && revision.backup.is_none() {
                    revision.backup = Some(crate::history::snapshot(
                        &revision.root,
                        &revision.path,
                        &source_bytes,
                    )?);
                }
                revision.applying = true;
                state::write_json(
                    &state::root()?.join("revisions").join(format!("{id}.json")),
                    &revision,
                )?;
            }
            let mut result = if recovered {
                json!({"path":revision.output_path,"sha256":revision.staged_sha256,"recovered":true,"previous_version":revision.backup})
            } else {
                writing::commit(
                    folders,
                    &folder.id,
                    &revision.output_path,
                    &bytes,
                    if revision.output_path == revision.path {
                        Some(&revision.source_sha256)
                    } else {
                        None
                    },
                )?
            };
            if revision.backup.is_none() {
                revision.backup = result["previous_version"].as_str().map(str::to_owned);
            }
            revision.status = "applied".into();
            revision.applying = false;
            state::write_json(
                &state::root()?.join("revisions").join(format!("{id}.json")),
                &revision,
            )?;
            if let Some(mut model) = revision.model.clone() {
                model.folder = folder.id.clone();
                model.path = revision.output_path.clone();
                let file = library::resolve(folders, &folder.id, &model.path)?;
                if let Err(error) = state::write_json(
                    &state::root()?
                        .join("models")
                        .join(format!("{}.json", state::scope(&file))),
                    &json!({"sha256":revision.staged_sha256,"args":model,"identity":revision.identity,"assets":revision.assets}),
                ) {
                    result["model_warning"] = json!(error);
                }
            }
            result
        }
        "undo" if revision.status == "applied" => {
            let path = advanced::input(folders, &folder.id, &revision.output_path)?;
            if writing::hash(&fs::read(&path).map_err(|e| e.to_string())?) != revision.staged_sha256
            {
                return Err(
                    "La sortie a changé : annulation refusée pour préserver vos modifications"
                        .into(),
                );
            }
            let result = if revision.output_path == revision.path {
                crate::history::restore(
                    folders,
                    &folder.id,
                    revision.backup.as_deref().ok_or("Sauvegarde absente")?,
                    &revision.staged_sha256,
                )?
            } else {
                writing::remove_created(
                    folders,
                    &folder.id,
                    &revision.output_path,
                    &revision.staged_sha256,
                )?
            };
            revision.status = "undone".into();
            result
        }
        _ => {
            return Err(
                "Action apply/reject sur une proposition ou undo sur une révision appliquée".into(),
            );
        }
    };
    state::write_json(
        &state::root()?.join("revisions").join(format!("{id}.json")),
        &revision,
    )?;
    Ok(result)
}

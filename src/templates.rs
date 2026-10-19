use crate::{
    advanced::{self, Asset},
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
    path::Path,
};
#[derive(Deserialize, schemars::JsonSchema)]
pub struct TemplateArgs {
    #[serde(default)]
    pub folder: String,
    pub template: String,
    pub output_path: String,
    /// Objet JSON : Typst accède à data ; Office remplace {{clé}} (valeurs scalaires).
    pub data: Value,
    #[serde(default)]
    pub assets: Vec<Asset>,
    pub collision_policy: Option<String>,
}
pub struct Prepared {
    pub source: String,
    pub assets: Vec<Asset>,
    pub data: Option<Value>,
}
pub fn prepare(
    folders: &[SharedFolder],
    folder: &str,
    source: &str,
    template: Option<&str>,
    data: Option<Value>,
    assets: Vec<Asset>,
) -> Result<Prepared, String> {
    let Some(template) = template else {
        if data.is_some() {
            return Err("data nécessite template".into());
        }
        return Ok(Prepared {
            source: source.into(),
            assets,
            data: None,
        });
    };
    if !source.trim().is_empty() {
        return Err("Fournir template ou source, pas les deux".into());
    }
    let full = advanced::input(folders, folder, template)?;
    if !template.to_lowercase().ends_with(".typ") {
        return Err("Modèle Typst .typ requis".into());
    }
    if fs::metadata(&full).map_err(|e| e.to_string())?.len() > 1_000_000 {
        return Err("Modèle limité à 1 Mo".into());
    }
    let text = fs::read_to_string(full).map_err(|e| e.to_string())?;
    let data = data.unwrap_or_else(|| json!({}));
    if !data.is_object() {
        return Err("data doit être un objet JSON".into());
    }
    let parent = Path::new(template).parent().unwrap_or(Path::new(""));
    let mut all = assets;
    let mut existing: BTreeSet<_> = all.iter().map(|a| a.name.to_lowercase()).collect();
    // Only assets next to an explicitly selected template in a dedicated subfolder.
    if !parent.as_os_str().is_empty() {
        let base = parent.to_string_lossy();
        let (paths, truncated) = crate::documents::paths(folders, folder, &base, true, None)?;
        if truncated {
            return Err("Dossier de modèles trop volumineux".into());
        }
        for (path, dir) in paths {
            if dir || path == template.replace('\\', "/") {
                continue;
            }
            let rel = Path::new(&path)
                .strip_prefix(parent)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let ext = Path::new(&path)
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if [
                "typ", "png", "jpg", "jpeg", "svg", "ttf", "otf", "json", "csv",
            ]
            .contains(&ext.as_str())
                && existing.insert(rel.to_lowercase())
            {
                all.push(Asset { path, name: rel });
            }
        }
    }
    if all.len() > 50 {
        return Err(
            "Maximum 50 assets par modèle ; séparer les modèles dans des sous-dossiers".into(),
        );
    }
    Ok(Prepared {
        source: format!("#let data = json(\"mcp-data.json\")\n{text}"),
        assets: all,
        data: Some(data),
    })
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn replacement(xml: &str, data: &Value) -> Result<(String, BTreeSet<String>), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let re = regex::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_.-]{0,63})\}\}").unwrap();
    let mut edits = Vec::new();
    let mut keys = BTreeSet::new();
    for p in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "p")
    {
        let nodes: Vec<_> = p
            .descendants()
            .filter(|n| {
                n.is_text()
                    && n.parent_element()
                        .is_some_and(|p| p.tag_name().name() == "t")
                    && n.ancestors()
                        .find(|a| a.is_element() && a.tag_name().name() == "p")
                        == Some(p)
            })
            .collect();
        let mut original = String::new();
        let mut offsets = Vec::new();
        let mut values = Vec::new();
        for n in &nodes {
            let text = n.text().unwrap_or("");
            offsets.push((original.len(), original.len() + text.len()));
            original.push_str(text);
            values.push(text.to_owned());
        }
        for captures in re
            .captures_iter(&original)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let m = captures.get(0).unwrap();
            let key = captures.get(1).unwrap().as_str();
            keys.insert(key.to_owned());
            let value = match data
                .get(key)
                .ok_or_else(|| format!("Valeur du modèle manquante : {key}"))?
            {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                v if v.is_number() || v.is_boolean() => v.to_string(),
                _ => return Err(format!("Office : valeur scalaire attendue pour {key}")),
            };
            if value.len() > 128000
                || value
                    .chars()
                    .any(|c| c.is_control() && !['\n', '\r', '\t'].contains(&c))
            {
                return Err("Valeur de modèle trop longue ou invalide".into());
            }
            let mut inserted = false;
            for (i, (start, end)) in offsets.iter().enumerate() {
                let from = m.start().max(*start);
                let to = m.end().min(*end);
                if from < to {
                    values[i].replace_range(
                        from - start..to - start,
                        if inserted {
                            ""
                        } else {
                            inserted = true;
                            &value
                        },
                    );
                }
            }
        }
        for (node, text) in nodes.iter().zip(values) {
            if text != node.text().unwrap_or("") {
                edits.push((node.range(), escape(&text)));
            }
        }
    }
    let mut result = xml.to_owned();
    edits.sort_by_key(|e| e.0.start);
    for (range, text) in edits.into_iter().rev() {
        result.replace_range(range, &text);
    }
    Ok((result, keys))
}
pub fn office(
    folders: &[SharedFolder],
    folder: &str,
    template: &str,
    output: &str,
    data: &Value,
) -> Result<Value, String> {
    writing::destination(folders, folder, output)?;
    if !data.is_object() || data.to_string().len() > 1_000_000 {
        return Err("data doit être un objet JSON de moins de 1 Mo".into());
    }
    let input = advanced::input(folders, folder, template)?;
    let ext = Path::new(template)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let out_ext = Path::new(output)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if !((out_ext == "docx" && ["docx", "dotx"].contains(&ext.as_str()))
        || (out_ext == "pptx" && ["pptx", "potx"].contains(&ext.as_str())))
    {
        return Err("Modèle DOCX/DOTX vers DOCX, ou PPTX/POTX vers PPTX".into());
    }
    let bytes = fs::read(input).map_err(|e| e.to_string())?;
    let mut zip = crate::office::archive(&bytes)?;
    let mut output_zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut replaced = BTreeSet::new();
    let mut raw_parts = Vec::new();
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_owned();
        library::validate_relative(&name)?;
        if Path::new(&name).is_absolute() || crate::history::reserved(Path::new(&name)) {
            return Err("Chemin OPC du modèle interdit".into());
        }
        if (name.ends_with(".xml") || name.ends_with(".rels")) && file.size() > 8 * 1024 * 1024 {
            return Err("Partie XML du modèle trop volumineuse".into());
        }
        if name.to_lowercase().contains("vbaproject") || name.contains("/embeddings/") {
            return Err("Modèles avec macros ou objets incorporés interdits".into());
        }
        let mut raw = Vec::new();
        file.read_to_end(&mut raw).map_err(|e| e.to_string())?;
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let xml = std::str::from_utf8(&raw).map_err(|e| e.to_string())?;
            let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
            if name.ends_with(".rels")
                && doc.descendants().any(|n| {
                    n.attribute("TargetMode") == Some("External")
                        && !n.attribute("Type").unwrap_or("").ends_with("/hyperlink")
                })
            {
                return Err("Relation externe du modèle interdite".into());
            }
            let (mut text, keys) = replacement(xml, data)?;
            replaced.extend(keys);
            if name == "[Content_Types].xml" {
                text = text
                    .replace(
                        "wordprocessingml.template.main+xml",
                        "wordprocessingml.document.main+xml",
                    )
                    .replace(
                        "presentationml.template.main+xml",
                        "presentationml.presentation.main+xml",
                    );
            }
            raw = text.into_bytes();
        }
        raw_parts.push((name, raw));
    }
    for (name, raw) in raw_parts {
        output_zip
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(|e| e.to_string())?;
        output_zip.write_all(&raw).map_err(|e| e.to_string())?;
    }
    let result = output_zip.finish().map_err(|e| e.to_string())?.into_inner();
    let mut value = writing::commit(folders, folder, output, &result, None)?;
    value["template"] = json!(template);
    value["replaced_keys"] = json!(replaced);
    Ok(value)
}
pub async fn create(folders: Vec<SharedFolder>, args: TemplateArgs) -> Result<Value, String> {
    if args.template.to_lowercase().ends_with(".typ") {
        let p = prepare(
            &folders,
            &args.folder,
            "",
            Some(&args.template),
            Some(args.data),
            args.assets,
        )?;
        crate::pdf_design::create_with_data(
            &folders,
            &args.folder,
            &args.output_path,
            &p.source,
            &p.assets,
            args.collision_policy.as_deref().unwrap_or("rename"),
            p.data.as_ref(),
        )
        .await
    } else {
        tokio::task::spawn_blocking(move || {
            office(
                &folders,
                &args.folder,
                &args.template,
                &args.output_path,
                &args.data,
            )
        })
        .await
        .map_err(|_| "Génération du modèle interrompue")?
    }
}
pub fn inspect(folders: &[SharedFolder], folder: &str, path: &str) -> Result<Value, String> {
    let full = advanced::input(folders, folder, path)?;
    let mut keys = BTreeSet::new();
    if path.to_lowercase().ends_with(".typ") {
        let source = fs::read_to_string(full).map_err(|e| e.to_string())?;
        let re = regex::Regex::new(r"data\.([A-Za-z_][A-Za-z0-9_]*)").unwrap();
        for caps in re.captures_iter(&source) {
            keys.insert(caps[1].to_owned());
        }
        Ok(
            json!({"template":path,"format":"typst","referenced_data_keys":keys,"assets_base":Path::new(path).parent(),"note":"Clés repérées statiquement ; voir le source pour les structures/tableaux imbriqués."}),
        )
    } else {
        let bytes = fs::read(full).map_err(|e| e.to_string())?;
        let mut zip = crate::office::archive(&bytes)?;
        let re = regex::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_.-]{0,63})\}\}").unwrap();
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if !f.name().ends_with(".xml") || f.size() > 8 * 1024 * 1024 {
                continue;
            }
            let mut xml = String::new();
            f.read_to_string(&mut xml).map_err(|e| e.to_string())?;
            let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
            for p in doc
                .descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "p")
            {
                let text = p
                    .descendants()
                    .filter(|n| {
                        n.is_text()
                            && n.parent_element()
                                .is_some_and(|p| p.tag_name().name() == "t")
                    })
                    .filter_map(|n| n.text())
                    .collect::<String>();
                for caps in re.captures_iter(&text) {
                    keys.insert(caps[1].to_owned());
                }
            }
        }
        Ok(
            json!({"template":path,"placeholders":keys,"syntax":"{{key}}","values":"strings, numbers, booleans or null"}),
        )
    }
}

pub fn listing(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    offset: usize,
    limit: usize,
) -> Result<Value, String> {
    library::select_folder(folders, folder)?;
    library::validate_relative(path)?;
    if library::resolve(folders, folder, path).is_err() {
        return Ok(
            json!({"templates":[],"note":"Créer ce sous-dossier dans votre partage et y déposer les modèles .typ, .dotx, .docx, .potx, .pptx ou .xlsx.","path":path}),
        );
    }
    let (paths, truncated) = crate::documents::paths(folders, folder, path, true, None)?;
    let templates: Vec<_> = paths
        .into_iter()
        .filter(|(p, d)| {
            !d && Path::new(p).extension().is_some_and(|e| {
                ["typ", "dotx", "docx", "potx", "pptx", "xlsx"]
                    .contains(&e.to_string_lossy().to_lowercase().as_str())
            })
        })
        .map(|(p, _)| p)
        .collect();
    let total = templates.len();
    let limit = limit.clamp(1, 200);
    Ok(
        json!({"templates":templates.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"total":total,"next_offset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit)),"tree_truncated":truncated}),
    )
}

use calamine::Reader;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::Path,
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

pub fn archive(bytes: &[u8]) -> Result<ZipArchive<Cursor<&[u8]>>, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("Archive Office invalide : {e}"))?;
    if archive.len() > 10_000 {
        return Err("Archive trop complexe".into());
    }
    let mut total = 0_u64;
    for i in 0..archive.len() {
        total = total.saturating_add(archive.by_index(i).map_err(|e| e.to_string())?.size());
    }
    if total > 100 * 1024 * 1024 {
        return Err("Contenu décompressé limité à 100 Mo".into());
    }
    Ok(archive)
}
fn xml_part(archive: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<String, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|_| format!("Partie Office manquante : {name}"))?;
    if file.size() > 8 * 1024 * 1024 {
        return Err("Partie XML trop volumineuse".into());
    }
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| e.to_string())?;
    Ok(text)
}
fn paragraphs(xml: &str) -> Result<String, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    Ok(doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "p")
        .map(|p| {
            p.descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "t")
                .filter_map(|n| n.text())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n"))
}
fn slide_parts(archive: &mut ZipArchive<Cursor<&[u8]>>) -> Result<Vec<String>, String> {
    let presentation = xml_part(archive, "ppt/presentation.xml")?;
    let rels = xml_part(archive, "ppt/_rels/presentation.xml.rels")?;
    let doc = roxmltree::Document::parse(&presentation).map_err(|e| e.to_string())?;
    let relationships = roxmltree::Document::parse(&rels).map_err(|e| e.to_string())?;
    let mut parts = Vec::new();
    for slide in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "sldId")
    {
        let id = slide
            .attribute((
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                "id",
            ))
            .ok_or("Relation de diapositive absente")?;
        let relation = relationships
            .descendants()
            .find(|n| n.attribute("Id") == Some(id))
            .ok_or("Relation de diapositive introuvable")?;
        let target = relation.attribute("Target").ok_or("Cible absente")?;
        let part = if target.starts_with('/') {
            target.trim_start_matches('/').to_owned()
        } else {
            format!("ppt/{target}")
        };
        if !part.starts_with("ppt/slides/") || part.contains("..") || !part.ends_with(".xml") {
            return Err("Relation de diapositive non prise en charge".into());
        }
        parts.push(part);
    }
    if parts.len() > 500 {
        return Err("Présentation limitée à 500 diapositives".into());
    }
    Ok(parts)
}

pub fn read(path: &Path, page: usize) -> Result<(String, Value), String> {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ["xlsx", "xls", "xlsb", "xlsm", "ods"].contains(&ext.as_str()) {
        if ext != "xls" && ext != "xlsb" {
            archive(&fs::read(path).map_err(|e| e.to_string())?)?;
        }
        let mut workbook =
            calamine::open_workbook_auto(path).map_err(|e| format!("Classeur illisible : {e}"))?;
        let sheets = workbook.sheet_names().to_vec();
        let sheet = sheets
            .get(page.checked_sub(1).ok_or("La première feuille est 1")?)
            .ok_or("Feuille inexistante")?;
        let range = workbook.worksheet_range(sheet).map_err(|e| e.to_string())?;
        let (rows, columns) = range.get_size();
        let text = range
            .rows()
            .take(5000)
            .map(|row| {
                row.iter()
                    .take(100)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n");
        return Ok((
            text,
            json!({"sheets":sheets,"sheet":sheet,"page":page,"pages":sheets.len(),"rows":rows,"columns":columns,"truncated":rows>5000||columns>100,"warning":"Valeurs mises en cache ; les formules ne sont pas recalculées par le lecteur."}),
        ));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut archive = archive(&bytes)?;
    match ext.as_str() {
        "docx" => Ok((
            paragraphs(&xml_part(&mut archive, "word/document.xml")?)?,
            json!({"warning":"Texte du corps du document, sans rendu des images ni des en-têtes."}),
        )),
        "pptx" => {
            let parts = slide_parts(&mut archive)?;
            let part = parts
                .get(page.checked_sub(1).ok_or("La première diapositive est 1")?)
                .ok_or("Diapositive inexistante")?;
            Ok((
                paragraphs(&xml_part(&mut archive, part)?)?,
                json!({"page":page,"pages":parts.len(),"warning":"Texte de la diapositive ; graphiques et images non décrits."}),
            ))
        }
        _ => Err("Format Office non pris en charge".into()),
    }
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
pub fn replace_text(
    bytes: &[u8],
    ext: &str,
    find: &str,
    replacement: &str,
    expected: usize,
) -> Result<(Vec<u8>, usize), String> {
    if find.is_empty()
        || find.len() > 64_000
        || replacement.len() > 128_000
        || expected == 0
        || expected > 1000
    {
        return Err("Remplacement invalide ou trop volumineux".into());
    }
    let mut source = archive(bytes)?;
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let mut count = 0;
    for i in 0..source.len() {
        let mut file = source.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_owned();
        let editable = (ext == "docx" && name == "word/document.xml")
            || (ext == "pptx"
                && name.starts_with("ppt/slides/slide")
                && name.ends_with(".xml")
                && !name.contains("_rels"));
        if !editable {
            output.raw_copy_file(file).map_err(|e| e.to_string())?;
            continue;
        }
        if file.size() > 8 * 1024 * 1024 {
            return Err("XML trop volumineux".into());
        }
        let mut xml = String::new();
        file.read_to_string(&mut xml).map_err(|e| e.to_string())?;
        let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
        let mut edits = Vec::new();
        for node in doc.descendants().filter(|n| {
            n.is_text()
                && n.parent_element()
                    .is_some_and(|p| p.tag_name().name() == "t")
        }) {
            let text = node.text().unwrap_or("");
            let matches = text.matches(find).count();
            if matches > 0 {
                count += matches;
                edits.push((node.range(), escaped(&text.replace(find, replacement))));
            }
        }
        for (range, text) in edits.into_iter().rev() {
            xml.replace_range(range, &text);
        }
        output
            .start_file(
                name,
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(|e| e.to_string())?;
        output
            .write_all(xml.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    if count != expected {
        return Err(format!(
            "{count} occurrence(s) trouvée(s), {expected} attendue(s). Un texte coupé entre plusieurs segments de mise en forme doit être remplacé segment par segment."
        ));
    }
    Ok((
        output.finish().map_err(|e| e.to_string())?.into_inner(),
        count,
    ))
}

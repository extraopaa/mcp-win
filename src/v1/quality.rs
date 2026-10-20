use crate::{advanced, library::SharedFolder};
use rmcp::schemars;
use serde_json::{Value, json};
use std::fs;
#[derive(serde::Deserialize, rmcp::schemars::JsonSchema)]
pub struct ReviewArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub pages: Option<Vec<usize>>,
    pub detailed: Option<bool>,
}
pub fn compact(report: &Value) -> Value {
    json!({"passed":report["passed"],"errors":report["errors"],"warnings":report["warnings"],"page_count":report["pages"].as_array().map_or(0,Vec::len),"visual_review_required":true})
}
pub fn inspect_bytes(bytes: &[u8], ext: &str) -> Result<Value, String> {
    let mut errors = vec![];
    let mut warnings = vec![];
    let mut pages = vec![];
    if ext == "pdf" {
        use pdfium_render::prelude::*;
        let _lock = advanced::PDF_LOCK.lock().map_err(|_| "PDF occupé")?;
        let engine = advanced::bind_pdfium()?;
        let doc = engine
            .load_pdf_from_byte_slice(bytes, None)
            .map_err(|e| e.to_string())?;
        let count = doc.pages().len();
        if count == 0 || count > 200 {
            errors.push("Nombre de pages PDF hors limites (1–200)".to_string())
        }
        for (i, page) in doc.pages().iter().take(200).enumerate() {
            let text = page.text().map_err(|e| e.to_string())?.all();
            let mut outside = 0;
            for object in page.objects().iter() {
                if object.as_text_object().is_some() {
                    if let Ok(b) = object.bounds() {
                        if b.left().value < -2.0
                            || b.bottom().value < -2.0
                            || b.right().value > page.width().value + 2.0
                            || b.top().value > page.height().value + 2.0
                        {
                            outside += 1
                        }
                    }
                }
            }
            if outside > 0 {
                errors.push(format!(
                    "Page {} : {outside} objet(s) texte hors page",
                    i + 1
                ))
            }
            if text.trim().is_empty() {
                warnings.push(format!(
                    "Page {} sans texte extractible : contrôler les images ou une page vide",
                    i + 1
                ))
            }
            if i < 3 {
                page.render_with_config(&PdfRenderConfig::new().set_target_width(600))
                    .map_err(|e| format!("Rendu impossible : {e}"))?;
            }
            pages.push(json!({"page":i+1,"characters":text.chars().count(),"width_pt":page.width().value,"height_pt":page.height().value,"text_outside_page":outside}));
        }
    } else if ["docx", "pptx", "xlsx"].contains(&ext) {
        let parts = super::design::unpack(bytes)?;
        let required = match ext {
            "docx" => "word/document.xml",
            "pptx" => "ppt/presentation.xml",
            _ => "xl/workbook.xml",
        };
        if !parts.contains_key(required) {
            errors.push(format!("Partie obligatoire absente : {required}"))
        }
        for (name, bytes) in &parts {
            if name.ends_with(".xml") || name.ends_with(".rels") {
                if let Err(e) = roxmltree::Document::parse(
                    std::str::from_utf8(bytes).map_err(|e| e.to_string())?,
                ) {
                    errors.push(format!("{name} : {e}"))
                }
            }
        }
        warnings.push("Contrôle structurel OOXML effectué ; le rendu Office dépend du lecteur et doit être vérifié visuellement.".into());
    } else {
        return Err("Contrôle qualité : PDF, DOCX, PPTX ou XLSX".into());
    }
    Ok(
        json!({"passed":errors.is_empty(),"errors":errors,"warnings":warnings,"pages":pages,"visual_review_required":true,"scope":"structure, rendu PDF initial et limites des objets texte ; ne garantit pas l’esthétique ni l’absence de chevauchements"}),
    )
}
pub fn inspect(folders: &[SharedFolder], folder: &str, path: &str) -> Result<Value, String> {
    let full = advanced::input(folders, folder, path)?;
    let ext = full
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    inspect_bytes(&fs::read(full).map_err(|e| e.to_string())?, &ext)
}

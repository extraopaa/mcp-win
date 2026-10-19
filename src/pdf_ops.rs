use crate::{advanced, library::SharedFolder, writing};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct PdfInput {
    pub path: String,
    /// Pages 1-based : ex. 1,3,5-7. Omettre pour toutes.
    pub pages: Option<String>,
}
pub fn combine(
    folders: &[SharedFolder],
    folder: &str,
    inputs: &[PdfInput],
    output: &str,
    flatten: bool,
) -> Result<Value, String> {
    writing::destination(folders, folder, output)?;
    if !output.to_lowercase().ends_with(".pdf") || inputs.is_empty() || inputs.len() > 50 {
        return Err("Sortie PDF, 1 à 50 sources".into());
    }
    let _guard = advanced::PDF_LOCK.lock().map_err(|_| "PDF indisponible")?;
    let pdfium = advanced::bind_pdfium()?;
    let mut result = pdfium.create_new_pdf().map_err(|e| e.to_string())?;
    for input in inputs {
        let mut source = pdfium
            .load_pdf_from_file(&advanced::input(folders, folder, &input.path)?, None)
            .map_err(|e| e.to_string())?;
        let has_fields = source.pages().iter().any(|p| {
            p.annotations().iter().any(|a| {
                a.annotation_type() == pdfium_render::prelude::PdfPageAnnotationType::Widget
            })
        });
        if has_fields && !flatten {
            return Err("Source contenant des formulaires : utiliser flatten_forms=true pour figer explicitement leur apparence avant fusion/extraction".into());
        }
        if has_fields {
            for mut page in source.pages_mut().iter() {
                page.flatten().map_err(|e| e.to_string())?;
            }
        }
        if source.pages().len() > 2000 {
            return Err("Source limitée à 2000 pages".into());
        }
        let at = result.pages().len();
        if let Some(pages) = &input.pages {
            validate_pages(pages, source.pages().len())?;
            result
                .pages_mut()
                .copy_pages_from_document(&source, pages, at)
                .map_err(|e| e.to_string())?;
        } else {
            result
                .pages_mut()
                .append(&source)
                .map_err(|e| e.to_string())?;
        }
        if result.pages().len() > 2000 {
            return Err("Sortie limitée à 2000 pages".into());
        }
    }
    let count = result.pages().len();
    let mut bytes = result.save_to_bytes().map_err(|e| e.to_string())?;
    if flatten {
        let mut doc = lopdf::Document::load_mem(&bytes).map_err(|e| e.to_string())?;
        for (_, id) in doc.get_pages() {
            let page = doc.get_dictionary(id).map_err(|e| e.to_string())?;
            if let Ok(annots) = page.get(b"Annots") {
                let (_, annots) = doc.dereference(annots).map_err(|e| e.to_string())?;
                for annot in annots.as_array().map_err(|e| e.to_string())? {
                    let (_, a) = doc.dereference(annot).map_err(|e| e.to_string())?;
                    if a.as_dict().is_ok_and(|d| {
                        d.get(b"Subtype")
                            .is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"Widget"))
                    }) {
                        return Err(
                            "Aplatissement incomplet : widget encore présent, sortie refusée"
                                .into(),
                        );
                    }
                }
            }
        }
        doc.catalog_mut()
            .map_err(|e| e.to_string())?
            .remove(b"AcroForm");
        bytes.clear();
        doc.save_to(&mut bytes).map_err(|e| e.to_string())?;
    }
    let mut value = writing::commit(folders, folder, output, &bytes, None)?;
    value["pages"] = json!(count);
    value["forms_flattened"] = json!(flatten);
    Ok(value)
}
fn validate_pages(pages: &str, count: i32) -> Result<(), String> {
    if pages.len() > 5000 {
        return Err("Sélection trop longue".into());
    }
    let mut total = 0;
    for part in pages.split(',') {
        let nums: Vec<_> = part
            .split('-')
            .map(|s| s.trim().parse::<i32>())
            .collect::<Result<_, _>>()
            .map_err(|_| "Pages attendues : 1,3,5-7")?;
        if nums.is_empty()
            || nums.len() > 2
            || nums[0] < 1
            || *nums.last().unwrap() > count
            || *nums.last().unwrap() < nums[0]
        {
            return Err("Pages hors limites".into());
        }
        total += nums.last().unwrap() - nums[0] + 1;
        if total > 2000 {
            return Err("2000 pages maximum".into());
        }
    }
    Ok(())
}
pub fn bookmarks(folders: &[SharedFolder], folder: &str, path: &str) -> Result<Vec<Value>, String> {
    let _guard = advanced::PDF_LOCK.lock().map_err(|_| "PDF indisponible")?;
    let pdfium = advanced::bind_pdfium()?;
    let source = pdfium
        .load_pdf_from_file(&advanced::input(folders, folder, path)?, None)
        .map_err(|e| e.to_string())?;
    Ok(source.bookmarks().iter().take(200).map(|b|json!({"title":b.title(),"page":b.destination().and_then(|d|d.page_index().ok()).map(|p|p+1)})).collect())
}

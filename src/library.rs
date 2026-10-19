use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedFolder {
    pub id: String,
    pub path: PathBuf,
    #[serde(default)]
    pub writable: bool,
}

impl SharedFolder {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into()
    }
}

pub fn select_folder<'a>(
    folders: &'a [SharedFolder],
    folder: &str,
) -> Result<&'a SharedFolder, String> {
    if let Some(found) = folders.iter().find(|f| f.id == folder) {
        return Ok(found);
    }
    if folder.is_empty() && folders.len() == 1 {
        return Ok(&folders[0]);
    }
    let matches: Vec<_> = folders.iter().filter(|f| f.name() == folder).collect();
    if matches.len() == 1 {
        return Ok(matches[0]);
    }
    let available: Vec<_> = folders
        .iter()
        .map(|f| json!({"id":f.id,"name":f.name(),"writable":f.writable}))
        .collect();
    Err(format!(
        "Dossier inconnu ou ambigu. Appelez list_folders et relancez l'outil avec un id retourné (writable=true pour créer). Ne demandez pas cet identifiant à l'utilisateur : il est accessible par MCP. Dossiers disponibles : {}",
        json!(available)
    ))
}

#[cfg(test)]
mod folder_selection_tests {
    use super::*;
    #[test]
    fn discovers_single_folder_and_reports_ambiguity_without_bypassing_write_permissions() {
        let temp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "actual-id".into(),
            path: temp.path().into(),
            writable: false,
        }];
        assert_eq!(select_folder(&folders, "").unwrap().id, "actual-id");
        assert_eq!(
            select_folder(&folders, &folders[0].name()).unwrap().id,
            "actual-id"
        );
        assert!(
            crate::writing::destination(&folders, "", "report.pdf")
                .unwrap_err()
                .contains("désactivée")
        );
        let error = select_folder(&folders, "guessed-id").unwrap_err();
        assert!(
            error.contains("list_folders")
                && error.contains("actual-id")
                && error.contains("writable")
        );
        let mut two = folders.clone();
        two.push(SharedFolder {
            id: "second-id".into(),
            ..folders[0].clone()
        });
        assert!(select_folder(&two, "").is_err());
        assert!(select_folder(&two, &folders[0].name()).is_err());
        assert_eq!(select_folder(&two, "second-id").unwrap().id, "second-id");
    }
}

pub fn validate_relative(relative: &str) -> Result<(), String> {
    if relative.contains([':', '\0']) {
        return Err("Chemin interdit (flux alternatifs inclus)".into());
    }
    for part in relative.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let numbered = stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"));
        if part == ".."
            || part.ends_with(['.', ' '])
            || ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"].contains(&stem.as_str())
            || numbered.is_some_and(|n| {
                ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
            })
        {
            return Err("Nom de fichier Windows ambigu ou réservé".into());
        }
    }
    Ok(())
}

pub fn resolve(folders: &[SharedFolder], folder: &str, relative: &str) -> Result<PathBuf, String> {
    validate_relative(relative)?;
    let root = select_folder(folders, folder)?;
    let root = root
        .path
        .canonicalize()
        .map_err(|_| "Dossier partagé inaccessible")?;
    if relative.contains(':') || relative.contains('\0') {
        return Err("Chemin interdit (flux alternatifs inclus)".into());
    }
    let relative = Path::new(relative);
    if crate::history::reserved(relative)
        || root
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case(crate::history::DIRECTORY))
    {
        return Err("L'historique local est réservé à la restauration".into());
    }
    if relative.is_absolute()
        || relative.components().any(|c| {
            matches!(
                c,
                std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
                    | std::path::Component::ParentDir
            )
        })
    {
        return Err("Utilisez un chemin relatif sans '..'".into());
    }
    let target = root
        .join(relative)
        .canonicalize()
        .map_err(|_| "Fichier introuvable")?;
    if !target.starts_with(&root) {
        return Err("Accès hors du dossier partagé interdit".into());
    }
    if crate::history::reserved(target.strip_prefix(&root).unwrap()) {
        return Err("Accès à l'historique réservé".into());
    }
    Ok(target)
}

pub fn extract(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
    page: usize,
) -> Result<Value, String> {
    let target = resolve(folders, folder, relative)?;
    let info = fs::metadata(&target).map_err(|_| "Fichier inaccessible")?;
    if !info.is_file() {
        return Err("Ce chemin n'est pas un fichier".into());
    }
    if info.len() > 25 * 1024 * 1024 {
        return Err("Limite de 25 Mo par fichier".into());
    }
    let ext = target
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let sha256 = crate::writing::hash(&fs::read(&target).map_err(|e| e.to_string())?);
    let mut metadata = json!({});
    let text = if ext == "pdf" {
        let bytes = fs::read(&target).map_err(|_| "Lecture impossible")?;
        let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes)
            .map_err(|_| "PDF illisible, chiffré ou non pris en charge")?;
        let text = pages
            .get(page.checked_sub(1).ok_or("La première page est 1")?)
            .ok_or("Page inexistante")?
            .clone();
        metadata = json!({"page": page, "pages": pages.len(), "warning": if text.trim().is_empty() {Some("Aucun texte : OCR nécessaire pour les pages scannées. Utiliser ocr_file pour reconnaître cette page.")} else {None}});
        text
    } else if ["docx", "pptx", "xlsx", "xls", "xlsb", "xlsm", "ods"].contains(&ext.as_str()) {
        let (text, meta) = crate::office::read(&target, page)?;
        metadata = meta;
        text
    } else {
        if ![
            "svg", "txt", "md", "csv", "tsv", "json", "xml", "html", "css", "js", "ts", "py", "rs",
            "log", "yaml", "yml", "toml", "typ",
        ]
        .contains(&ext.as_str())
        {
            return Err("Format non pris en charge".into());
        }
        let text =
            fs::read_to_string(&target).map_err(|_| "Le fichier doit être un texte UTF-8")?;
        if text.contains('\0') {
            return Err("Fichier binaire non pris en charge".into());
        }
        if ext == "svg" {
            let document = roxmltree::Document::parse(&text)
                .map_err(|_| "SVG XML invalide ou DTD interdit")?;
            let svg = document.root_element();
            if svg.tag_name().name() != "svg" {
                return Err("Racine SVG absente".into());
            }
            metadata = json!({"width": svg.attribute("width"), "height": svg.attribute("height"), "viewBox": svg.attribute("viewBox"),
                "warning": "Code SVG brut, sans exécution de scripts ni analyse visuelle."});
        }
        text
    };
    Ok(json!({"path":relative,"format":ext,"sha256":sha256,"metadata":metadata,"text":text}))
}

pub fn read(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
    page: usize,
    offset: usize,
    limit: usize,
) -> Result<Value, String> {
    let mut value = extract(folders, folder, relative, page)?;
    let text = value["text"].as_str().unwrap_or("");
    let total = text.chars().count();
    let segment: String = text
        .chars()
        .skip(offset)
        .take(limit.clamp(1, 50000))
        .collect();
    let end = offset.saturating_add(segment.chars().count());
    value["text"] = json!(segment);
    value["total_characters"] = json!(total);
    value["next_offset"] = json!((end < total).then_some(end));
    Ok(value)
}

pub fn render_svg(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
) -> Result<Vec<u8>, String> {
    let target = resolve(folders, folder, relative)?;
    if target
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("svg"))
    {
        return Err("Choisissez un SVG".into());
    }
    if fs::metadata(&target)
        .map_err(|_| "Fichier inaccessible")?
        .len()
        > 5 * 1024 * 1024
    {
        return Err("SVG limité à 5 Mo pour le rendu".into());
    }
    let source = fs::read_to_string(&target).map_err(|_| "SVG illisible")?;
    let mut options = resvg::usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    options.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_str(&source, &options)
        .map_err(|e| format!("SVG invalide : {e}"))?;
    let size = tree.size();
    let scale = (1600.0_f32 / size.width().max(size.height())).min(1.0);
    let width = (size.width() * scale).ceil().max(1.0) as u32;
    let height = (size.height() * scale).ceil().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or("Image trop grande")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_a_real_pdf_page() {
        let temp = tempfile::tempdir().unwrap();
        let stream = "BT /F1 18 Tf 50 750 Td (Bonjour PDF) Tj ET";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_owned(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
            format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        ];
        let mut pdf = "%PDF-1.4\n".to_owned();
        let mut offsets = vec![0];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
        }
        let xref = pdf.len();
        pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            pdf.push_str(&format!("{offset:010} 00000 n \n"));
        }
        pdf.push_str(&format!(
            "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        ));
        fs::write(temp.path().join("sample.pdf"), pdf).unwrap();
        let folders = vec![SharedFolder {
            writable: false,
            id: "test".into(),
            path: temp.path().into(),
        }];
        let value = read(&folders, "test", "sample.pdf", 1, 0, 1000).unwrap();
        assert!(value["text"].as_str().unwrap().contains("Bonjour PDF"));
        assert_eq!(value["metadata"]["pages"], 1);
        assert!(read(&folders, "test", "sample.pdf", 2, 0, 100).is_err());
    }
    #[test]
    fn reads_unicode_in_segments_and_rejects_escape() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("note.txt"), "été ☀ bonjour").unwrap();
        let folders = vec![SharedFolder {
            writable: false,
            id: "one".into(),
            path: temp.path().into(),
        }];
        let value = read(&folders, "one", "note.txt", 1, 0, 5).unwrap();
        assert_eq!(value["text"], "été ☀");
        assert_eq!(value["next_offset"], 5);
        assert!(resolve(&folders, "one", "../secret.txt").is_err());
        assert!(resolve(&folders, "one", "C:\\Windows\\win.ini").is_err());
        assert!(resolve(&folders, "unknown", "note.txt").is_err());
    }
    #[test]
    fn svg_is_data_and_invalid_xml_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(
            temp.path().join("a.svg"),
            r#"<svg viewBox="0 0 10 10"><text>Salut</text></svg>"#,
        )
        .unwrap();
        let folders = vec![SharedFolder {
            writable: false,
            id: "one".into(),
            path: temp.path().into(),
        }];
        assert_eq!(
            read(&folders, "one", "a.svg", 1, 0, 1000).unwrap()["metadata"]["viewBox"],
            "0 0 10 10"
        );
        let png = render_svg(&folders, "one", "a.svg").unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        fs::write(
            temp.path().join("a.svg"),
            "<!DOCTYPE svg [<!ENTITY x SYSTEM 'file:///secret'>]><svg>&x;</svg>",
        )
        .unwrap();
        assert!(read(&folders, "one", "a.svg", 1, 0, 1000).is_err());
    }
}

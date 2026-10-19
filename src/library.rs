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

pub fn resolve(folders: &[SharedFolder], folder: &str, relative: &str) -> Result<PathBuf, String> {
    let root = folders
        .iter()
        .find(|f| f.id == folder)
        .ok_or("Dossier partagé inconnu")?;
    let root = root
        .path
        .canonicalize()
        .map_err(|_| "Dossier partagé inaccessible")?;
    let relative = Path::new(relative);
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
    Ok(target)
}

pub fn list(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
    offset: usize,
) -> Result<Value, String> {
    let dir = resolve(folders, folder, relative)?;
    let mut items = fs::read_dir(dir).map_err(|_| "Dossier inaccessible")?
        .filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| !t.is_symlink()))
        .map(|e| json!({"name": e.file_name().to_string_lossy(), "path": Path::new(relative).join(e.file_name()).to_string_lossy().replace('\\', "/"),
            "directory": e.file_type().is_ok_and(|t| t.is_dir())})).collect::<Vec<_>>();
    items.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    let total = items.len();
    let entries: Vec<_> = items.into_iter().skip(offset).take(100).collect();
    Ok(
        json!({"entries": entries, "total": total, "next_offset": (offset.saturating_add(100) < total).then_some(offset.saturating_add(100))}),
    )
}

pub fn read(
    folders: &[SharedFolder],
    folder: &str,
    relative: &str,
    page: usize,
    offset: usize,
    limit: usize,
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
    let mut metadata = json!({});
    let text = if ext == "pdf" {
        let bytes = fs::read(&target).map_err(|_| "Lecture impossible")?;
        let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes)
            .map_err(|_| "PDF illisible, chiffré ou non pris en charge")?;
        let text = pages
            .get(page.checked_sub(1).ok_or("La première page est 1")?)
            .ok_or("Page inexistante")?
            .clone();
        metadata = json!({"page": page, "pages": pages.len(), "warning": if text.trim().is_empty() {Some("Aucun texte : OCR nécessaire pour les pages scannées. OCR non inclus.")} else {None}});
        text
    } else {
        if ![
            "svg", "txt", "md", "csv", "tsv", "json", "xml", "html", "css", "js", "ts", "py", "rs",
            "log", "yaml", "yml", "toml",
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
    let total = text.chars().count();
    let segment: String = text
        .chars()
        .skip(offset)
        .take(limit.clamp(1, 50_000))
        .collect();
    let end = offset.saturating_add(segment.chars().count());
    Ok(
        json!({"path": relative, "format": ext, "metadata": metadata, "text": segment, "total_characters": total, "next_offset": (end < total).then_some(end)}),
    )
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

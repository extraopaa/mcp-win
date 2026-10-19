use crate::{
    library::{self, SharedFolder},
    office, writing,
};

use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

pub fn engine_path(name: &str) -> Result<PathBuf, String> {
    let adjacent = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .unwrap()
        .join("engines")
        .join(name);
    if adjacent.is_file() {
        return Ok(adjacent);
    }
    #[cfg(debug_assertions)]
    {
        let development = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("vendor/engines/runtime")
            .join(name);
        if development.is_file() {
            return Ok(development);
        }
    }
    Err(format!(
        "Moteur manquant : engines/{name}. Extraire l'archive complète de l'application."
    ))
}

pub fn input(folders: &[SharedFolder], folder: &str, path: &str) -> Result<PathBuf, String> {
    let path = library::resolve(folders, folder, path)?;
    let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > 25 * 1024 * 1024 {
        return Err("Fichier requis, maximum 25 Mo".into());
    }
    Ok(path)
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct Asset {
    /// Chemin relatif du fichier source dans le même dossier partagé.
    pub path: String,
    /// Nom dans le projet Typst ou chemin de partie Office, ex. logo.png / word/media/logo.png.
    pub name: String,
}
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 240
        && !name.contains(['\\', ':', '\0'])
        && name
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

#[cfg(test)]
pub async fn typst(
    folders: &[SharedFolder],
    folder: &str,
    output: &str,
    source: &str,
    assets: &[Asset],
) -> Result<Value, String> {
    crate::pdf_design::create(folders, folder, output, source, assets, "rename").await
}

pub struct CompiledPdf {
    pub bytes: Vec<u8>,
    pub warnings: String,
    pub adjustments: Vec<String>,
    pub source: String,
}

pub async fn compile_typst(
    folders: &[SharedFolder],
    folder: &str,
    source: &str,
    assets: &[Asset],
) -> Result<CompiledPdf, String> {
    static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _slot = SLOTS
        .acquire()
        .await
        .map_err(|_| "Compilation indisponible")?;
    library::select_folder(folders, folder)?;
    if source.len() > 1_000_000 || assets.len() > 50 {
        return Err("Source <= 1 Mo, maximum 50 assets".into());
    }
    let (source, adjustments) = crate::pdf_design::normalize(source);
    // No registry/package downloads: dependencies must be explicit local assets.
    if source.contains("@preview/") || source.contains("@local/") {
        return Err(
            "Paquets distants désactivés : fournir les fichiers .typ comme assets locaux".into(),
        );
    }
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let root = temp.path();
    fs::write(root.join("main.typ"), &source).map_err(|e| e.to_string())?;
    let mut total = source.len() as u64;
    let mut names = std::collections::HashSet::new();
    for asset in assets {
        let name = asset.name.replace('\\', "/");
        if !safe_name(&name)
            || [
                "main.typ",
                "output.pdf",
                "errors.txt",
                "lato-regular.ttf",
                "lato-bold.ttf",
                "lato-italic.ttf",
                "lato-bolditalic.ttf",
            ]
            .contains(&name.to_lowercase().as_str())
            || !names.insert(name.to_lowercase())
        {
            return Err("Nom d'asset invalide ou réservé".into());
        }
        let src = input(folders, folder, &asset.path)?;
        total += fs::metadata(&src).map_err(|e| e.to_string())?.len();
        if total > 50 * 1024 * 1024 {
            return Err("Assets limités à 50 Mo".into());
        }
        let dst = root.join(&name);
        fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::copy(src, dst).map_err(|e| e.to_string())?;
    }
    fs::write(
        root.join("Lato-Regular.ttf"),
        include_bytes!("../assets/fonts/Lato-Regular.ttf"),
    )
    .map_err(|e| e.to_string())?;
    for (name, bytes) in [
        (
            "Lato-Bold.ttf",
            include_bytes!("../assets/fonts/Lato-Bold.ttf").as_slice(),
        ),
        (
            "Lato-Italic.ttf",
            include_bytes!("../assets/fonts/Lato-Italic.ttf").as_slice(),
        ),
        (
            "Lato-BoldItalic.ttf",
            include_bytes!("../assets/fonts/Lato-BoldItalic.ttf").as_slice(),
        ),
    ] {
        fs::write(root.join(name), bytes).map_err(|e| e.to_string())?;
    }
    let mut command = tokio::process::Command::new(engine_path("typst.exe")?);
    command
        .args([
            "compile",
            "--root",
            ".",
            "--font-path",
            ".",
            "--ignore-system-fonts",
            "--package-path",
            "packages",
            "--package-cache-path",
            "packages",
            "main.typ",
            "output.pdf",
        ])
        .current_dir(root)
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .env("NO_PROXY", "")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(
            fs::File::create(root.join("errors.txt")).map_err(|e| e.to_string())?,
        ))
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let _job = crate::process_job::ProcessJob::attach(&child)?;
    let status = match tokio::time::timeout(std::time::Duration::from_secs(60), child.wait()).await
    {
        Ok(status) => status.map_err(|e| e.to_string())?,
        Err(_) => {
            let _ = child.kill().await;
            return Err("Compilation Typst interrompue après 60 secondes".into());
        }
    };
    let errors = fs::read_to_string(root.join("errors.txt"))
        .unwrap_or_default()
        .replace(root.to_string_lossy().as_ref(), "<projet>");
    if !status.success() {
        return Err(format!(
            "Typst : {}",
            errors.chars().take(12000).collect::<String>()
        ));
    }
    let bytes = fs::read(root.join("output.pdf")).map_err(|e| e.to_string())?;
    if bytes.len() > 25 * 1024 * 1024 {
        return Err("PDF limité à 25 Mo".into());
    }
    Ok(CompiledPdf {
        bytes,
        warnings: errors.chars().take(12000).collect(),
        adjustments,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn designed_pdf_render_and_scanned_ocr_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "qa".into(),
            path: tmp.path().into(),
            writable: true,
        }];
        let source = include_str!("../docs/examples/designed.typ");
        typst(&folders, "qa", "designed.pdf", source, &[])
            .await
            .unwrap();
        let (image, pages) = pdf_image(&folders, "qa", "designed.pdf", 1, 1600).unwrap();
        assert_eq!(pages, 1);
        let bytes = png(&image).unwrap();
        fs::write(tmp.path().join("scan.png"), &bytes).unwrap();
        let recognized = crate::ocr::recognize(
            folders.clone(),
            crate::ocr::OcrArgs {
                folder: "qa".into(),
                path: "scan.png".into(),
                page: None,
                languages: None,
                layout: None,
                rotate: None,
            },
        )
        .await
        .unwrap();
        assert!(
            recognized["text"].as_str().unwrap().contains("MCP"),
            "{recognized}"
        );
        assert!(
            recognized["text"].as_str().unwrap().contains("identité"),
            "French accents: {recognized}"
        );
        assert!(!recognized["lines"].as_array().unwrap().is_empty());
        typst(
            &folders,
            "qa",
            "scan.pdf",
            "#set page(margin: 0pt)\n#image(\"scan.png\", width: 100%)",
            &[Asset {
                path: "scan.png".into(),
                name: "scan.png".into(),
            }],
        )
        .await
        .unwrap();
        let scanned = crate::ocr::recognize(
            folders.clone(),
            crate::ocr::OcrArgs {
                folder: "qa".into(),
                path: "scan.pdf".into(),
                page: None,
                languages: None,
                layout: None,
                rotate: None,
            },
        )
        .await
        .unwrap();
        assert!(
            scanned["text"].as_str().unwrap().contains("MCP"),
            "{scanned}"
        );
        assert_eq!(scanned["pages"], 1);
        assert!(pdf_image(&folders, "qa", "scan.pdf", 0, 1000).is_err());
        assert!(
            typst(&folders, "qa", "bad.pdf", "#read(\"../secret.txt\")", &[])
                .await
                .is_err()
        );
        assert!(
            typst(
                &folders,
                "qa",
                "bad.pdf",
                "#import \"@preview/foo:1.0.0\"",
                &[]
            )
            .await
            .is_err()
        );
        if let Some(dir) = std::env::var_os("MCP_WIN_QA_OUTPUT") {
            let dir = PathBuf::from(dir);
            fs::create_dir_all(&dir).unwrap();
            fs::copy(tmp.path().join("designed.pdf"), dir.join("designed.pdf")).unwrap();
            fs::write(dir.join("designed.png"), bytes).unwrap();
            fs::write(
                dir.join("ocr.json"),
                serde_json::to_vec_pretty(&recognized).unwrap(),
            )
            .unwrap();
        }
    }
    #[test]
    fn office_package_styles_copy_and_validation() {
        let tmp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "qa".into(),
            path: tmp.path().into(),
            writable: true,
        }];
        let xml = include_str!("../docs/examples/document.xml");
        package(
            &folders,
            "qa",
            "styled.docx",
            None,
            None,
            &[PackagePart {
                name: "word/document.xml".into(),
                text: Some(xml.into()),
                asset_path: None,
            }],
            &[],
        )
        .unwrap();
        let p = inspect_package(&folders, "qa", "styled.docx", Some("word/document.xml")).unwrap();
        assert!(p["text"].as_str().unwrap().contains("w:color"));
        let before = fs::read(tmp.path().join("styled.docx")).unwrap();
        package(
            &folders,
            "qa",
            "copy.docx",
            Some("styled.docx"),
            p["sha256"].as_str(),
            &[PackagePart {
                name: "word/document.xml".into(),
                text: Some(xml.replace("MCP Win", "Nouvelle édition")),
                asset_path: None,
            }],
            &[],
        )
        .unwrap();
        assert_eq!(before, fs::read(tmp.path().join("styled.docx")).unwrap());
        assert!(
            package(
                &folders,
                "qa",
                "invalid.docx",
                None,
                None,
                &[PackagePart {
                    name: "word/document.xml".into(),
                    text: Some("<bad>".into()),
                    asset_path: None
                }],
                &[]
            )
            .is_err()
        );
        assert!(
            package(
                &folders,
                "qa",
                "bad.docx",
                None,
                None,
                &[PackagePart {
                    name: "../escape.xml".into(),
                    text: Some("<x/>".into()),
                    asset_path: None
                }],
                &[]
            )
            .is_err()
        );
        if let Some(dir) = std::env::var_os("MCP_WIN_QA_OUTPUT") {
            fs::create_dir_all(&dir).unwrap();
            fs::write(PathBuf::from(dir).join("styled.docx"), before).unwrap();
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PackagePart {
    /// Chemin OPC, ex. word/document.xml, word/styles.xml, ppt/slides/slide1.xml.
    pub name: String,
    /// XML complet pour une partie XML. Exactement un de text / asset_path.
    pub text: Option<String>,
    /// Fichier binaire dans le dossier partagé pour une image, police ou autre ressource.
    pub asset_path: Option<String>,
}
pub fn inspect_package(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    part: Option<&str>,
) -> Result<Value, String> {
    let bytes = fs::read(input(folders, folder, path)?).map_err(|e| e.to_string())?;
    let mut archive = office::archive(&bytes)?;
    if let Some(name) = part {
        let file = archive.by_name(name).map_err(|e| e.to_string())?;
        if file.size() > 1_000_000 {
            return Err("Partie limitée à 1 Mo".into());
        }
        let mut text = String::new();
        file.take(1_000_001)
            .read_to_string(&mut text)
            .map_err(|_| "Partie binaire : utiliser read_file_bytes pour l'archive")?;
        return Ok(json!({"sha256":writing::hash(&bytes), "part":name,"text":text}));
    }
    let mut parts = Vec::new();
    for i in 0..archive.len() {
        let f = archive.by_index(i).map_err(|e| e.to_string())?;
        parts.push(json!({"name":f.name(),"bytes":f.size()}));
    }
    Ok(json!({"sha256":writing::hash(&bytes),"parts":parts}))
}
pub fn package(
    folders: &[SharedFolder],
    folder: &str,
    output: &str,
    source: Option<&str>,
    expected: Option<&str>,
    edits: &[PackagePart],
    remove: &[String],
) -> Result<Value, String> {
    writing::destination(folders, folder, output)?;
    let ext = Path::new(output)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["docx", "pptx", "xlsx"].contains(&ext.as_str()) || edits.len() > 200 || remove.len() > 200
    {
        return Err("DOCX/PPTX/XLSX uniquement ; 200 parties par opération".into());
    }
    let mut parts = BTreeMap::new();
    if let Some(source) = source {
        if Path::new(source)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase()
            != ext
        {
            return Err("Extensions source et sortie différentes".into());
        }
        let bytes = fs::read(input(folders, folder, source)?).map_err(|e| e.to_string())?;
        if expected != Some(writing::hash(&bytes).as_str()) {
            return Err("Empreinte source absente ou périmée".into());
        }
        let mut zip = office::archive(&bytes)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            parts.insert(f.name().to_owned(), bytes);
        }
    } else if ext == "docx" {
        let bytes = crate::generate::docx("", &[])?;
        let mut zip = office::archive(&bytes)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            let mut b = Vec::new();
            f.read_to_end(&mut b).map_err(|e| e.to_string())?;
            parts.insert(f.name().to_owned(), b);
        }
    }
    for name in remove {
        parts.remove(name);
    }
    for edit in edits {
        if !safe_name(&edit.name) {
            return Err("Chemin de partie invalide".into());
        }
        let bytes = match (&edit.text, &edit.asset_path) {
            (Some(text), None) if text.len() <= 2_000_000 => text.as_bytes().to_vec(),
            (None, Some(path)) => {
                fs::read(input(folders, folder, path)?).map_err(|e| e.to_string())?
            }
            _ => return Err("Fournir exactement text (<=2 Mo) ou asset_path".into()),
        };
        parts.insert(edit.name.clone(), bytes);
    }
    let required = match ext.as_str() {
        "docx" => "word/document.xml",
        "pptx" => "ppt/presentation.xml",
        _ => "xl/workbook.xml",
    };
    if !parts.contains_key("[Content_Types].xml")
        || !parts.contains_key("_rels/.rels")
        || !parts.contains_key(required)
    {
        return Err("Parties OPC obligatoires manquantes".into());
    }
    let mut total = 0usize;
    for (name, bytes) in &parts {
        total = total.saturating_add(bytes.len());
        if total > 50 * 1024 * 1024
            || !safe_name(name)
            || name.to_lowercase().contains("vbaproject")
            || name.contains("/embeddings/")
        {
            return Err(
                "Archive trop volumineuse, chemin invalide, macro ou objet incorporé interdit"
                    .into(),
            );
        }
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let xml = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
            let doc = roxmltree::Document::parse(xml).map_err(|e| format!("{name}: {e}"))?;
            if name.ends_with(".rels")
                && doc.descendants().any(|n| {
                    n.attribute("TargetMode") == Some("External")
                        && !n.attribute("Type").unwrap_or("").ends_with("/hyperlink")
                })
            {
                return Err("Relations externes autorisées uniquement pour les hyperliens".into());
            }
        }
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    let bytes = zip.finish().map_err(|e| e.to_string())?.into_inner();
    writing::commit(folders, folder, output, &bytes, None)
}

pub fn pdf_image(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    page: usize,
    width: u32,
) -> Result<(image::DynamicImage, usize), String> {
    use pdfium_render::prelude::*;
    static PDF_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = PDF_LOCK.lock().map_err(|_| "Rendu PDF indisponible")?;
    if page == 0 || !(400..=2400).contains(&width) {
        return Err("Page >=1, largeur entre 400 et 2400 pixels".into());
    }
    let pdfium = match Pdfium::bind_to_library(engine_path("pdfium.dll")?) {
        Ok(bindings) => Pdfium::new(bindings),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Pdfium::default(),
        Err(e) => return Err(e.to_string()),
    };
    let document = pdfium
        .load_pdf_from_file(&input(folders, folder, path)?, None)
        .map_err(|e| e.to_string())?;
    let count = document.pages().len() as usize;
    let index = i32::try_from(page - 1).map_err(|_| "Page hors limites")?;
    let page = document.pages().get(index).map_err(|e| e.to_string())?;
    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(width as i32)
                .set_maximum_height(3200),
        )
        .map_err(|e| e.to_string())?;
    Ok((bitmap.as_image().map_err(|e| e.to_string())?, count))
}
pub fn png(image: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}

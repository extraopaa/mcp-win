use crate::{advanced, library::SharedFolder, writing};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::Value;
use std::{fs, io::Read};
#[derive(Deserialize, schemars::JsonSchema)]
pub struct PatchArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub expected_sha256: String,
    /// Patch unifié standard, contexte et positions exacts ; pas de réécriture intégrale.
    pub patch: String,
    /// Pour OOXML seulement : nom XML de la partie, ex. word/document.xml.
    pub part: Option<String>,
    /// Requis pour OOXML : copie de sortie de même extension. L’original reste intact.
    pub output_path: Option<String>,
}
pub fn apply(folders: &[SharedFolder], args: PatchArgs) -> Result<Value, String> {
    if args.patch.len() > 256000 {
        return Err("Patch limité à 256 Ko".into());
    }
    let full = advanced::input(folders, &args.folder, &args.path)?;
    let bytes = fs::read(full).map_err(|e| e.to_string())?;
    if writing::hash(&bytes) != args.expected_sha256 {
        return Err("Conflit : empreinte source périmée".into());
    }
    let patch = diffy::Patch::from_str(&args.patch).map_err(|e| e.to_string())?;
    if let Some(part) = args.part {
        if !(part.ends_with(".xml") || part.ends_with(".rels")) {
            return Err("Patch Office : partie XML requise".into());
        }
        let mut zip = crate::office::archive(&bytes)?;
        let mut text = String::new();
        let file = zip.by_name(&part).map_err(|e| e.to_string())?;
        if file.size() > 2_000_000 {
            return Err("Partie limitée à 2 Mo".into());
        }
        file.take(2_000_001)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        let modified = diffy::apply(&text, &patch)
            .map_err(|e| format!("Patch incompatible avec la source : {e}"))?;
        advanced::package(
            folders,
            &args.folder,
            args.output_path
                .as_deref()
                .ok_or("output_path requis pour Office")?,
            Some(&args.path),
            Some(&args.expected_sha256),
            &[advanced::PackagePart {
                name: part,
                text: Some(modified),
                asset_path: None,
            }],
            &[],
        )
    } else {
        if args.output_path.as_ref().is_some_and(|p| p != &args.path) {
            return Err("Patch texte : modifie path ; omettre output_path".into());
        }
        if bytes.len() > 2_000_000 {
            return Err("Texte limité à 2 Mo".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| "Texte UTF-8 requis")?;
        let modified = diffy::apply(text, &patch)
            .map_err(|e| format!("Patch incompatible avec la source : {e}"))?;
        writing::text(
            folders,
            &args.folder,
            &args.path,
            &modified,
            Some(&args.expected_sha256),
        )
    }
}

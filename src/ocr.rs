use crate::{advanced, library::SharedFolder};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OcrArgs {
    pub folder: String,
    pub path: String,
    /// Première page = 1 ; une page par appel.
    pub page: Option<usize>,
    /// fra+eng (défaut), fra ou eng. Modèles fournis hors ligne.
    pub languages: Option<String>,
    /// auto (défaut), block (bloc uniforme), sparse (texte dispersé).
    pub layout: Option<String>,
    /// Rotation horaire avant reconnaissance : 0, 90, 180, 270 degrés.
    pub rotate: Option<u16>,
}

fn prepare(
    folders: &[SharedFolder],
    args: &OcrArgs,
) -> Result<(image::DynamicImage, usize), String> {
    let source = advanced::input(folders, &args.folder, &args.path)?;
    let page = args.page.unwrap_or(1);
    let (image, pages) = if source
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        advanced::pdf_image(folders, &args.folder, &args.path, page, 2000)?
    } else {
        if page != 1 {
            return Err("Image : utiliser page=1".into());
        }
        let mut reader = image::ImageReader::open(source)
            .map_err(|e| e.to_string())?
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(12000);
        limits.max_image_height = Some(12000);
        limits.max_alloc = Some(160 * 1024 * 1024);
        reader.limits(limits);
        (reader.decode().map_err(|e| e.to_string())?, 1)
    };
    let image = match args.rotate.unwrap_or(0) {
        0 => image,
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => return Err("Rotation attendue : 0,90,180,270".into()),
    };
    Ok((
        if image.width() > 2400 || image.height() > 3200 {
            image.thumbnail(2400, 3200)
        } else {
            image
        },
        pages,
    ))
}

pub async fn recognize(folders: Vec<SharedFolder>, args: OcrArgs) -> Result<Value, String> {
    static SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _slot = SLOTS.acquire().await.map_err(|_| "OCR indisponible")?;
    let languages = args.languages.as_deref().unwrap_or("fra+eng").to_owned();
    if !["fra+eng", "eng+fra", "fra", "eng"].contains(&languages.as_str()) {
        return Err("Langues fournies : fra, eng, fra+eng".into());
    }
    let psm = match args.layout.as_deref().unwrap_or("auto") {
        "auto" => "3",
        "block" => "6",
        "sparse" => "11",
        _ => return Err("layout : auto, block ou sparse".into()),
    };
    let page = args.page.unwrap_or(1);
    let (image, pages) = tokio::task::spawn_blocking(move || prepare(&folders, &args))
        .await
        .map_err(|_| "Préparation image échouée")??;
    let (width, height) = (image.width(), image.height());
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let image_path = temp.path().join("input.png");
    fs::write(&image_path, advanced::png(&image)?).map_err(|e| e.to_string())?;
    let executable = advanced::engine_path("tesseract/tesseract.exe")?;
    let data = executable.parent().unwrap().join("tessdata");
    let mut command = tokio::process::Command::new(&executable);
    command
        .arg(&image_path)
        .arg("output")
        .arg("--tessdata-dir")
        .arg(data)
        .args([
            "-l",
            &languages,
            "--psm",
            psm,
            "--dpi",
            "240",
            "-c",
            "tessedit_create_tsv=1",
        ])
        .current_dir(temp.path())
        .env("OMP_THREAD_LIMIT", "2")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(
            fs::File::create(temp.path().join("error.txt")).map_err(|e| e.to_string())?,
        ))
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let _job = crate::process_job::ProcessJob::attach(&child)?;
    let status = match tokio::time::timeout(std::time::Duration::from_secs(60), child.wait()).await
    {
        Ok(s) => s.map_err(|e| e.to_string())?,
        Err(_) => {
            let _ = child.kill().await;
            return Err("OCR interrompu après 60 secondes".into());
        }
    };
    if !status.success() {
        return Err(format!(
            "Tesseract : {}",
            fs::read_to_string(temp.path().join("error.txt"))
                .unwrap_or_default()
                .chars()
                .take(8000)
                .collect::<String>()
        ));
    }
    let file = temp.path().join("output.tsv");
    if fs::metadata(&file).map_err(|e| e.to_string())?.len() > 5_000_000 {
        return Err("Résultat OCR trop volumineux".into());
    }
    let tsv = fs::read_to_string(file).map_err(|e| e.to_string())?;
    let mut groups: BTreeMap<(u32, u32, u32), Vec<Value>> = BTreeMap::new();
    let mut words = Vec::new();
    for row in tsv.lines().skip(1) {
        let fields: Vec<_> = row.splitn(12, '\t').collect();
        if fields.len() != 12 || fields[0] != "5" || fields[11].trim().is_empty() {
            continue;
        }
        let number = |i: usize| fields[i].parse::<u32>().unwrap_or(0);
        let confidence = fields[10].parse::<f64>().unwrap_or(-1.0);
        let word = json!({"text":fields[11],"confidence":confidence,"box":{"x":number(6),"y":number(7),"width":number(8),"height":number(9)}});
        groups
            .entry((number(2), number(3), number(4)))
            .or_default()
            .push(word.clone());
        words.push(word);
        if words.len() > 10000 {
            return Err("Page limitée à 10 000 mots".into());
        }
    }
    let lines:Vec<_>=groups.values().map(|words| {
        let n=|v:&Value,key:&str|v["box"][key].as_u64().unwrap_or(0);
        let x=words.iter().map(|w|n(w,"x")).min().unwrap_or(0);let y=words.iter().map(|w|n(w,"y")).min().unwrap_or(0);
        let right=words.iter().map(|w|n(w,"x")+n(w,"width")).max().unwrap_or(x);let bottom=words.iter().map(|w|n(w,"y")+n(w,"height")).max().unwrap_or(y);
        json!({"text":words.iter().filter_map(|w|w["text"].as_str()).collect::<Vec<_>>().join(" "),"box":{"x":x,"y":y,"width":right-x,"height":bottom-y}})
    }).collect();
    let text = lines
        .iter()
        .filter_map(|l| l["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(
        json!({"text":text,"lines":lines,"words":words,"page":page,"pages":pages,"width":width,"height":height,"languages":languages,"engine":"tesseract-5.5.0","warning":"OCR imprimé : vérifier les valeurs critiques. Manuscrits et documents dégradés non garantis ; pas de reconstruction automatique des tableaux."}),
    )
}

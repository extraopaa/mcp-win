use crate::{advanced, library::SharedFolder};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

#[derive(Clone, Default, Deserialize, schemars::JsonSchema)]
pub struct OcrArgs {
    /// ID via list_folders ou nom exact unique. Omettre si un seul dossier est partage.
    #[serde(default)]
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
    /// Texte seul par défaut. false ajoute mots, boîtes et confiance (coûteux en tokens).
    pub plain_text_only: Option<bool>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    /// auto (défaut), fast ou accurate ; essais supplémentaires uniquement si utiles.
    pub quality: Option<String>,
    /// Correction de rotations à 90/180/270 si la lecture initiale est mauvaise.
    pub auto_rotate: Option<bool>,
    pub word_offset: Option<usize>,
    /// Métadonnées détaillées paginées : 1000 mots par défaut, 10000 maximum.
    pub word_limit: Option<usize>,
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
        advanced::pdf_image(folders, &args.folder, &args.path, page, 2400)?
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
    if !["auto", "fast", "accurate"].contains(&args.quality.as_deref().unwrap_or("auto")) {
        return Err("quality : auto, fast ou accurate".into());
    }
    let source = advanced::input(&folders, &args.folder, &args.path)?;
    let source_hash = crate::writing::hash_file(&source)?;
    let key = crate::writing::hash(
        format!(
            "{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}",
            source_hash,
            source.extension(),
            args.page,
            args.languages,
            args.layout,
            args.rotate,
            args.quality,
            args.auto_rotate
        )
        .as_bytes(),
    );
    if let Some(value) = cache()
        .lock()
        .unwrap()
        .iter()
        .find(|(k, _)| k == &key)
        .map(|(_, v)| v.clone())
    {
        return Ok(output(value, &args, true));
    }
    let page = args.page.unwrap_or(1);
    let prepare_args = args.clone();
    let (image, pages) = tokio::task::spawn_blocking(move || prepare(&folders, &prepare_args))
        .await
        .map_err(|_| "Préparation image échouée")??;
    let image = clean_image(image);
    let (width, height) = (image.width(), image.height());
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let image_path = temp.path().join("input.png");
    fs::write(&image_path, advanced::png(&image)?).map_err(|e| e.to_string())?;
    let started = std::time::Instant::now();
    let initial = run_pass(&image_path, temp.path(), &languages, psm).await?;
    let mut best = initial;
    let mut rotation = 0u16;
    let mut passes = 1;
    let (mut confidence, mut count) = score(&best);
    let quality = args.quality.as_deref().unwrap_or("auto");
    if quality != "fast" && (confidence < 75.0 || count < 8 || quality == "accurate") {
        let mut candidates = vec![(0u16, "6")];
        if args.rotate.is_none()
            && args.auto_rotate.unwrap_or(true)
            && (confidence < 65.0 || count < 8)
        {
            candidates.extend([(90, "3"), (270, "3"), (180, "3")]);
        }
        for (angle, mode) in candidates {
            if started.elapsed().as_secs() > 55 {
                break;
            }
            let candidate = match angle {
                90 => image.rotate90(),
                180 => image.rotate180(),
                270 => image.rotate270(),
                _ => image.clone(),
            };
            fs::write(&image_path, advanced::png(&candidate)?).map_err(|e| e.to_string())?;
            if let Ok(tsv) = run_pass(&image_path, temp.path(), &languages, mode).await {
                passes += 1;
                let (c, n) = score(&tsv);
                if c + (n.min(100) as f64) * 0.25 > confidence + (count.min(100) as f64) * 0.25 {
                    best = tsv;
                    confidence = c;
                    count = n;
                    rotation = angle;
                }
                if confidence >= 85.0 && count >= 8 {
                    break;
                }
            }
        }
    }
    let tsv = best;
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
    let mut plain_text = String::new();
    let mut paragraph = None;
    for ((block, par, _), words) in &groups {
        let key = (*block, *par);
        if !plain_text.is_empty() {
            plain_text.push_str(if paragraph == Some(key) { "\n" } else { "\n\n" });
        }
        plain_text.push_str(
            &words
                .iter()
                .filter_map(|w| w["text"].as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
        paragraph = Some(key);
    }
    if crate::writing::hash_file(&source)? != source_hash {
        return Err("Le fichier a changé pendant l’OCR ; relancez la lecture".into());
    }
    let (result_width, result_height) = if rotation == 90 || rotation == 270 {
        (height, width)
    } else {
        (width, height)
    };
    let result = json!({"text":text,"plain_text":plain_text,"lines":lines,"words":words,"page":page,"pages":pages,"width":result_width,"height":result_height,"languages":languages,"engine":"tesseract-5.5.0","confidence":confidence,"needs_review":confidence<75.0,"rotation_applied":(args.rotate.unwrap_or(0)+rotation)%360,"passes":passes,"sha256":source_hash});
    {
        let mut c = cache().lock().unwrap();
        c.push_back((key, result.clone()));
        while c.len() > 8
            || c.iter().map(|(_, v)| v.to_string().len()).sum::<usize>() > 32 * 1024 * 1024
        {
            c.pop_front();
        }
    }
    Ok(output(result, &args, false))
}

fn cache() -> &'static std::sync::Mutex<std::collections::VecDeque<(String, Value)>> {
    static CACHE: std::sync::OnceLock<
        std::sync::Mutex<std::collections::VecDeque<(String, Value)>>,
    > = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}
fn score(tsv: &str) -> (f64, usize) {
    let values: Vec<f64> = tsv
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<_> = l.splitn(12, '\t').collect();
            if f.len() == 12 && f[0] == "5" && !f[11].trim().is_empty() {
                f[10].parse::<f64>().ok().filter(|c| *c >= 0.0)
            } else {
                None
            }
        })
        .collect();
    (
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        },
        values.len(),
    )
}
fn clean_image(image: image::DynamicImage) -> image::DynamicImage {
    let rgba = image.to_rgba8();
    let mut gray = image::GrayImage::new(rgba.width(), rgba.height());
    let mut histogram = [0u64; 256];
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let lum = (299 * p[0] as u32 + 587 * p[1] as u32 + 114 * p[2] as u32) / 1000;
        let v = ((lum * a + 255 * (255 - a)) / 255) as u8;
        gray.put_pixel(x, y, image::Luma([v]));
        histogram[v as usize] += 1;
    }
    let count = gray.width() as u64 * gray.height() as u64;
    let mut sum = 0;
    let mut low = 0;
    let mut high = 255;
    for (i, n) in histogram.iter().enumerate() {
        sum += n;
        if sum < count / 100 {
            low = i
        }
        if sum < count * 99 / 100 {
            high = i
        }
    }
    let dark = histogram[..128].iter().sum::<u64>() > count * 3 / 4;
    for pixel in gray.pixels_mut() {
        let v = pixel[0] as usize;
        let normalized = if high > low + 20 {
            ((v.saturating_sub(low) * 255 / (high - low)).min(255)) as u8
        } else {
            v as u8
        };
        pixel[0] = if dark { 255 - normalized } else { normalized };
    }
    image::DynamicImage::ImageLuma8(gray)
}
fn output(mut result: Value, args: &OcrArgs, hit: bool) -> Value {
    let plain = args.plain_text_only.unwrap_or(true);
    let text = if plain {
        result["plain_text"].as_str()
    } else {
        result["text"].as_str()
    }
    .unwrap_or("");
    let total = text.chars().count();
    let offset = args.offset.unwrap_or(0);
    let segment: String = text
        .chars()
        .skip(offset)
        .take(
            args.limit
                .unwrap_or(crate::limits::DEFAULT_TEXT_CHARS)
                .clamp(1, 50000),
        )
        .collect();
    let end = offset + segment.chars().count();
    result["text"] = json!(segment);
    result["total_characters"] = json!(total);
    result["next_offset"] = json!((end < total).then_some(end));
    result["plain_text_only"] = json!(plain);
    result["cache_hit"] = json!(hit);
    result.as_object_mut().unwrap().remove("plain_text");
    if plain {
        for key in ["words", "lines", "width", "height", "languages", "engine"] {
            result.as_object_mut().unwrap().remove(key);
        }
    } else {
        let words = result["words"].as_array().cloned().unwrap_or_default();
        let start = args.word_offset.unwrap_or(0);
        let limit = args.word_limit.unwrap_or(1000).clamp(1, 10000);
        result["words"] = json!(words.iter().skip(start).take(limit).collect::<Vec<_>>());
        result["next_word_offset"] = json!(
            (start.saturating_add(limit) < words.len()).then_some(start.saturating_add(limit))
        );
        result["lines_truncated"] = json!(result["lines"].as_array().map_or(0, Vec::len) > limit);
        result["lines"] = json!(
            result["lines"]
                .as_array()
                .into_iter()
                .flatten()
                .take(limit)
                .collect::<Vec<_>>()
        );
    }
    result
}
async fn run_pass(
    image_path: &std::path::Path,
    directory: &std::path::Path,
    languages: &str,
    psm: &str,
) -> Result<String, String> {
    let executable = advanced::engine_path("tesseract/tesseract.exe")?;
    let data = executable.parent().unwrap().join("tessdata");
    let mut command = tokio::process::Command::new(&executable);
    command
        .arg(image_path)
        .arg("output")
        .arg("--tessdata-dir")
        .arg(data)
        .args([
            "-l",
            &languages,
            "--psm",
            psm,
            "--dpi",
            "300",
            "-c",
            "tessedit_create_tsv=1",
            "-c",
            if psm == "6" {
                "thresholding_method=2"
            } else {
                "thresholding_method=0"
            },
        ])
        .current_dir(directory)
        .env("OMP_THREAD_LIMIT", "2")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(
            fs::File::create(directory.join("error.txt")).map_err(|e| e.to_string())?,
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
            fs::read_to_string(directory.join("error.txt"))
                .unwrap_or_default()
                .chars()
                .take(8000)
                .collect::<String>()
        ));
    }
    let file = directory.join("output.tsv");
    if fs::metadata(&file).map_err(|e| e.to_string())?.len() > 5_000_000 {
        return Err("Résultat OCR trop volumineux".into());
    }
    fs::read_to_string(file).map_err(|e| e.to_string())
}

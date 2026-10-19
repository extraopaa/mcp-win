use crate::{
    advanced, history,
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

pub fn regex(pattern: &str, case_sensitive: bool) -> Result<regex::Regex, String> {
    if pattern.len() > 2000 {
        return Err("Regex limitée à 2000 octets".into());
    }
    regex::RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .size_limit(1024 * 1024)
        .build()
        .map_err(|e| e.to_string())
}
pub fn paths(
    folders: &[SharedFolder],
    folder: &str,
    base: &str,
    recursive: bool,
    pattern: Option<&str>,
) -> Result<(Vec<(String, bool)>, bool), String> {
    let shared = library::select_folder(folders, folder)?;
    let root = shared.path.canonicalize().map_err(|e| e.to_string())?;
    let start = library::resolve(folders, folder, base)?;
    if !start.is_dir() {
        return Err("Un dossier est requis".into());
    }
    let glob = pattern
        .map(|p| {
            if p.len() > 256 {
                return Err("Glob trop long".to_string());
            }
            globset::GlobBuilder::new(p)
                .literal_separator(true)
                .case_insensitive(true)
                .build()
                .map(|g| g.compile_matcher())
                .map_err(|e| e.to_string())
        })
        .transpose()?;
    let walker = walkdir::WalkDir::new(&start)
        .follow_links(false)
        .max_depth(if recursive { 64 } else { 1 })
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| {
            !e.file_type().is_symlink()
                && !history::reserved(e.path().strip_prefix(&root).unwrap_or(e.path()))
        });
    let mut files = Vec::new();
    let mut truncated = false;
    for (visited, e) in walker.enumerate() {
        if visited > 50000 {
            truncated = true;
            break;
        }
        let e = e.map_err(|e| e.to_string())?;
        if e.depth() == 0 {
            continue;
        }
        let relative = e
            .path()
            .strip_prefix(&root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        if glob
            .as_ref()
            .is_none_or(|g| g.is_match(e.path().strip_prefix(&start).unwrap()))
        {
            files.push((relative, e.file_type().is_dir()));
        }
        if files.len() >= 50000 {
            truncated = true;
            break;
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((files, truncated))
}
#[derive(Clone, Serialize)]
pub struct Unit {
    pub page: usize,
    pub sheet: Option<String>,
    pub text: String,
    pub line_offset: usize,
}
struct Cache {
    entries: HashMap<PathBuf, (String, Arc<Vec<Unit>>)>,
    bytes: usize,
}
static INDEX: OnceLock<Mutex<Cache>> = OnceLock::new();
pub fn units(folders: &[SharedFolder], folder: &str, path: &str) -> Result<Arc<Vec<Unit>>, String> {
    let full = advanced::input(folders, folder, path)?;
    let bytes = fs::read(&full).map_err(|e| e.to_string())?;
    let hash = writing::hash(&bytes);
    let cache = INDEX.get_or_init(|| {
        Mutex::new(Cache {
            entries: HashMap::new(),
            bytes: 0,
        })
    });
    if let Some((key, data)) = cache
        .lock()
        .map_err(|_| "Index indisponible")?
        .entries
        .get(&full)
    {
        if key == &hash {
            return Ok(data.clone());
        }
    }
    let ext = full
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let mut result = Vec::new();
    if ext == "pdf" {
        let pages =
            pdf_extract::extract_text_from_mem_by_pages(&bytes).map_err(|e| e.to_string())?;
        for (i, text) in pages.into_iter().enumerate() {
            result.push(Unit {
                line_offset: 0,
                page: i + 1,
                sheet: None,
                text,
            });
        }
    } else if ["xlsx", "xls", "xlsb", "xlsm", "ods"].contains(&ext.as_str()) {
        use calamine::Reader;
        if !["xls", "xlsb"].contains(&ext.as_str()) {
            crate::office::archive(&bytes)?;
        }
        let mut workbook = calamine::open_workbook_auto(&full).map_err(|e| e.to_string())?;
        let names = workbook.sheet_names().to_vec();
        if names.len() > 500 {
            return Err("Classeur limité à 500 feuilles pour l’indexation".into());
        }
        let mut total = 0usize;
        for (index, name) in names.iter().enumerate() {
            let range = workbook.worksheet_range(name).map_err(|e| e.to_string())?;
            let (rows, cols) = range.get_size();
            if rows.saturating_mul(cols) > 2_000_000 {
                return Err("Feuille limitée à 2 millions de cellules pour la recherche".into());
            }
            let mut text = String::new();
            for row in range.rows() {
                for (i, cell) in row.iter().enumerate() {
                    if i > 0 {
                        text.push('\t');
                    }
                    let value = cell.to_string();
                    total = total.saturating_add(value.len() + 1);
                    if total > 16 * 1024 * 1024 {
                        return Err("Texte du classeur limité à 16 Mo pour la recherche".into());
                    }
                    text.push_str(&value);
                }
                text.push('\n');
            }
            result.push(Unit {
                line_offset: range.start().map(|(row, _)| row as usize).unwrap_or(0),
                page: index + 1,
                sheet: Some(name.clone()),
                text,
            });
        }
    } else {
        let first = library::extract(folders, folder, path, 1)?;
        let count = if ["pptx", "xlsx", "xls", "xlsb", "xlsm", "ods"].contains(&ext.as_str()) {
            first["metadata"]["pages"].as_u64().unwrap_or(1).min(500) as usize
        } else {
            1
        };
        result.push(Unit {
            line_offset: 0,
            page: 1,
            sheet: first["metadata"]["sheet"].as_str().map(str::to_owned),
            text: first["text"].as_str().unwrap_or("").into(),
        });
        for page in 2..=count {
            let v = library::extract(folders, folder, path, page)?;
            result.push(Unit {
                line_offset: 0,
                page,
                sheet: v["metadata"]["sheet"].as_str().map(str::to_owned),
                text: v["text"].as_str().unwrap_or("").into(),
            });
        }
    }
    let size = result.iter().map(|u| u.text.len()).sum::<usize>();
    if size > 16 * 1024 * 1024 {
        return Err("Texte extrait limité à 16 Mo par document".into());
    }
    let result = Arc::new(result);
    let mut cache = cache.lock().map_err(|_| "Index indisponible")?;
    if cache.bytes + size > 64 * 1024 * 1024 || cache.entries.len() >= 512 {
        cache.entries.clear();
        cache.bytes = 0;
    }
    if let Some((_, old)) = cache.entries.remove(&full) {
        cache.bytes = cache
            .bytes
            .saturating_sub(old.iter().map(|u| u.text.len()).sum::<usize>());
    }
    cache.bytes += size;
    cache.entries.insert(full, (hash, result.clone()));
    Ok(result)
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    #[serde(default)]
    pub folder: String,
    pub query: String,
    pub use_regex: Option<bool>,
    pub case_sensitive: Option<bool>,
    pub glob_pattern: Option<String>,
    pub max_matches: Option<usize>,
    pub max_files: Option<usize>,
    pub cursor: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct Cursor {
    signature: String,
    file: usize,
    unit: usize,
    line: usize,
}
pub fn search(folders: &[SharedFolder], args: SearchArgs) -> Result<Value, String> {
    if args.query.is_empty() {
        return Err("Recherche vide".into());
    }
    let pattern = if args.use_regex.unwrap_or(false) {
        args.query.clone()
    } else {
        regex::escape(&args.query)
    };
    let re = regex(&pattern, args.case_sensitive.unwrap_or(false))?;
    let (files, tree_truncated) = paths(
        folders,
        &args.folder,
        "",
        true,
        args.glob_pattern.as_deref(),
    )?;
    let files: Vec<_> = files.into_iter().filter(|(_, dir)| !*dir).collect();
    let signature = writing::hash(
        format!(
            "{}|{}|{:?}|{}|{:?}",
            library::select_folder(folders, &args.folder)?.id,
            pattern,
            args.glob_pattern,
            args.case_sensitive.unwrap_or(false),
            files
        )
        .as_bytes(),
    );
    let mut at = if let Some(cursor) = args.cursor {
        serde_json::from_str::<Cursor>(&cursor).map_err(|_| "Curseur de recherche invalide")?
    } else {
        Cursor {
            signature: signature.clone(),
            file: 0,
            unit: 0,
            line: 0,
        }
    };
    if at.signature != signature {
        return Err("Recherche ou arborescence modifiée : recommencer sans cursor".into());
    }
    let max = args.max_matches.unwrap_or(25).clamp(1, 100);
    let budget = args.max_files.unwrap_or(200).clamp(1, 1000);
    let mut matches = Vec::new();
    let mut scanned = 0;
    let mut skipped = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while at.file < files.len() && scanned < budget && std::time::Instant::now() < deadline {
        let path = &files[at.file].0;
        let data = match units(folders, &args.folder, path) {
            Ok(v) => v,
            Err(e) => {
                if skipped.len() < 10 {
                    skipped.push(json!({"path":path,"reason":e}));
                }
                at.file += 1;
                at.unit = 0;
                at.line = 0;
                scanned += 1;
                continue;
            }
        };
        for unit_index in at.unit..data.len() {
            let unit = &data[unit_index];
            for (i, line) in unit
                .text
                .lines()
                .enumerate()
                .skip(if unit_index == at.unit { at.line } else { 0 })
            {
                if let Some(found) = re.find(line) {
                    let start = line[..found.start()].chars().count().saturating_sub(100);
                    let snippet: String = line.chars().skip(start).take(400).collect();
                    matches.push(json!({"path":path,"page":unit.page,"sheet":unit.sheet,"line":i+1+unit.line_offset,"snippet":snippet}));
                    if matches.len() >= max {
                        at.unit = unit_index;
                        at.line = i + 1;
                        return Ok(
                            json!({"matches":matches,"next_cursor":serde_json::to_string(&at).unwrap(),"scanned":scanned+1,"skipped":skipped,"tree_truncated":tree_truncated,"index":"local_memory_content_hash"}),
                        );
                    }
                }
            }
        }
        at.file += 1;
        at.unit = 0;
        at.line = 0;
        scanned += 1;
    }
    Ok(
        json!({"matches":matches,"next_cursor":if at.file<files.len(){Some(serde_json::to_string(&at).unwrap())}else{None},"scanned":scanned,"skipped":skipped,"tree_truncated":tree_truncated,"index":"local_memory_content_hash"}),
    )
}
pub fn line_excerpt(line: &str, pattern: Option<&regex::Regex>) -> String {
    if line.chars().count() <= 1000 || pattern.is_none() {
        return line.to_owned();
    }
    let re = pattern.unwrap();
    let mut excerpts = Vec::new();
    for found in re.find_iter(line).take(10) {
        let start = line[..found.start()].chars().count().saturating_sub(100);
        let text = line.chars().skip(start).take(500).collect::<String>();
        excerpts.push(format!("[col {}] …{}…", start + 1, text));
    }
    excerpts.join(" | ")
}
pub fn filtered_read(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    page: usize,
    range: Option<[usize; 2]>,
    pattern: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<Value, String> {
    let mut result = library::extract(folders, folder, path, page)?;
    let text = result["text"].as_str().unwrap_or("");
    let (from, to) = range.map(|[a, b]| (a, b)).unwrap_or((1, usize::MAX));
    if from == 0 || to < from {
        return Err("line_range : [début,fin] inclusifs à partir de 1".into());
    }
    let re = pattern.map(|p| regex(p, true)).transpose()?;
    let selected = text
        .lines()
        .enumerate()
        .filter(|(i, l)| i + 1 >= from && i + 1 <= to && re.as_ref().is_none_or(|r| r.is_match(l)))
        .map(|(i, l)| format!("{}: {}", i + 1, line_excerpt(l, re.as_ref())))
        .collect::<Vec<_>>()
        .join("\n");
    let total = selected.chars().count();
    let part: String = selected
        .chars()
        .skip(offset)
        .take(limit.clamp(1, 50000))
        .collect();
    let end = offset.saturating_add(part.chars().count());
    result["text"] = json!(part);
    result["total_characters"] = json!(total);
    result["next_offset"] = json!((end < total).then_some(end));
    Ok(result)
}
pub fn compact_table(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    sheet: usize,
) -> Result<Value, String> {
    use calamine::Reader;
    let full = advanced::input(folders, folder, path)?;
    let ext = full
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let (headers, samples, rows, columns, sheets) = if ["csv", "tsv"].contains(&ext.as_str()) {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .delimiter(if ext == "tsv" {
                b'\t'
            } else {
                use std::io::{BufRead, BufReader};
                let mut first = String::new();
                BufReader::new(fs::File::open(&full).map_err(|e| e.to_string())?)
                    .read_line(&mut first)
                    .map_err(|e| e.to_string())?;
                let mut quotes = false;
                let mut comma = 0;
                let mut semi = 0;
                for c in first.chars() {
                    match c {
                        '"' => quotes = !quotes,
                        ',' if !quotes => comma += 1,
                        ';' if !quotes => semi += 1,
                        _ => {}
                    }
                }
                if semi > comma { b';' } else { b',' }
            })
            .from_path(&full)
            .map_err(|e| e.to_string())?;
        let column_count = reader.headers().map_err(|e| e.to_string())?.len();
        let headers = reader
            .headers()
            .map_err(|e| e.to_string())?
            .iter()
            .take(100)
            .map(|v| v.chars().take(160).collect::<String>())
            .collect::<Vec<_>>();
        let mut samples = Vec::new();
        let mut rows = 0;
        for row in reader.records() {
            let row = row.map_err(|e| e.to_string())?;
            if rows < 3 {
                samples.push(
                    row.iter()
                        .take(100)
                        .map(|s| s.chars().take(200).collect::<String>())
                        .collect::<Vec<_>>(),
                );
            }
            rows += 1;
            if rows > 2_000_000 {
                return Err("CSV limité à 2 millions de lignes".into());
            }
        }
        (headers, samples, rows, column_count, vec!["CSV".to_owned()])
    } else {
        if !["xls", "xlsb"].contains(&ext.as_str()) {
            crate::office::archive(&fs::read(&full).map_err(|e| e.to_string())?)?;
        }
        let mut wb = calamine::open_workbook_auto(&full).map_err(|e| e.to_string())?;
        let sheets = wb.sheet_names().to_vec();
        let name = sheets
            .get(sheet.checked_sub(1).ok_or("Feuille >=1")?)
            .ok_or("Feuille inconnue")?;
        let range = wb.worksheet_range(name).map_err(|e| e.to_string())?;
        let (rows, columns) = range.get_size();
        let mut iter = range.rows();
        let headers = iter
            .next()
            .unwrap_or(&[])
            .iter()
            .take(100)
            .map(|v| v.to_string().chars().take(160).collect::<String>())
            .collect();
        let samples = iter
            .take(3)
            .map(|r| {
                r.iter()
                    .take(100)
                    .map(|c| c.to_string().chars().take(200).collect())
                    .collect::<Vec<_>>()
            })
            .collect();
        (headers, samples, rows.saturating_sub(1), columns, sheets)
    };
    let types: Vec<_> = (0..headers.len())
        .map(|i| {
            let vals: Vec<_> = samples
                .iter()
                .filter_map(|r| r.get(i))
                .filter(|v| !v.is_empty())
                .collect();
            if vals.is_empty() {
                "unknown"
            } else if vals.iter().all(|s| s.parse::<f64>().is_ok()) {
                "number"
            } else if vals
                .iter()
                .all(|s| ["true", "false", "TRUE", "FALSE"].contains(&s.as_str()))
            {
                "boolean"
            } else {
                "text"
            }
        })
        .collect();
    Ok(
        json!({"path":path,"sheets":sheets,"sheet":sheet,"columns":headers,"inferred_types_from_sample":types,"sample_rows":samples,"data_rows":rows,"column_count":columns,"header_assumed_first_row":true}),
    )
}
pub fn outline(folders: &[SharedFolder], folder: &str, path: &str) -> Result<Value, String> {
    let full = advanced::input(folders, folder, path)?;
    let ext = full
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if ["xlsx", "xls", "xlsb", "xlsm", "ods"].contains(&ext.as_str()) {
        use calamine::Reader;
        let wb = calamine::open_workbook_auto(full).map_err(|e| e.to_string())?;
        return Ok(json!({"format":ext,"sheets":wb.sheet_names()}));
    }
    let data = units(folders, folder, path)?;
    let words = data
        .iter()
        .map(|u| u.text.split_whitespace().count())
        .sum::<usize>();
    let mut headings = Vec::new();
    if ext == "md" {
        let mut fence = false;
        for (i, line) in data[0].text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fence = !fence;
            }
            if !fence {
                let level = line.chars().take_while(|c| *c == '#').count();
                if (1..=3).contains(&level) && line.as_bytes().get(level) == Some(&b' ') {
                    headings.push(json!({"level":level,"title":line[level+1..].chars().take(160).collect::<String>(),"line":i+1}));
                }
            }
        }
    } else if ext == "docx" {
        let bytes = fs::read(full).map_err(|e| e.to_string())?;
        let mut zip = crate::office::archive(&bytes)?;
        let mut xml = String::new();
        use std::io::Read;
        zip.by_name("word/document.xml")
            .map_err(|e| e.to_string())?
            .take(8 * 1024 * 1024)
            .read_to_string(&mut xml)
            .map_err(|e| e.to_string())?;
        let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
        for (i, p) in doc
            .descendants()
            .filter(|n| {
                n.has_tag_name((
                    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
                    "p",
                ))
            })
            .enumerate()
        {
            let style = p
                .descendants()
                .find(|n| n.tag_name().name() == "pStyle")
                .and_then(|n| {
                    n.attributes()
                        .find(|a| a.name() == "val")
                        .map(|a| a.value().to_lowercase())
                });
            let level = p
                .descendants()
                .find(|n| n.tag_name().name() == "outlineLvl")
                .and_then(|n| n.attributes().find(|a| a.name() == "val"))
                .and_then(|a| a.value().parse::<usize>().ok())
                .map(|n| n + 1)
                .or_else(|| {
                    style.and_then(|s| {
                        s.trim_start_matches("heading")
                            .trim_start_matches("titre")
                            .parse()
                            .ok()
                    })
                });
            if let Some(level) = level.filter(|n| *n <= 3) {
                let title = p
                    .descendants()
                    .filter(|n| n.tag_name().name() == "t")
                    .filter_map(|n| n.text())
                    .collect::<String>();
                headings.push(json!({"level":level,"title":title.chars().take(160).collect::<String>(),"paragraph":i+1}));
            }
        }
    } else if ext == "pdf" {
        headings = crate::pdf_ops::bookmarks(folders, folder, path)?;
    } else if ext == "pptx" {
        for u in data.iter() {
            headings.push(json!({"title":u.text.lines().next().unwrap_or("").chars().take(160).collect::<String>(),"slide":u.page}));
        }
    }
    let total = headings.len();
    headings.truncate(40);
    Ok(
        json!({"format":ext,"pages_or_units":data.len(),"words":words,"headings":headings,"headings_total":total,"truncated":total>40,"pdf_headings_from_bookmarks":ext=="pdf"}),
    )
}

pub fn listing(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    recursive: bool,
    pattern: Option<&str>,
    compact: bool,
    offset: usize,
    limit: usize,
) -> Result<Value, String> {
    let (files, truncated) = paths(folders, folder, path, recursive, pattern)?;
    let total = files.len();
    let limit = limit.clamp(1, 500);
    let selected = files.into_iter().skip(offset).take(limit);
    let entries: Vec<Value> = if compact {
        selected
            .map(|(p, d)| json!(if d { format!("{p}/") } else { p }))
            .collect()
    } else {
        selected
            .map(|(p, d)| {
                let metadata = library::resolve(folders, folder, &p)
                    .and_then(|path| fs::metadata(path).map_err(|e| e.to_string()))
                    .ok();
                json!({"path":p,"directory":d,"bytes":metadata.map(|m|m.len())})
            })
            .collect()
    };
    Ok(
        json!({"entries":entries,"total":total,"next_offset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit)),"tree_truncated":truncated,"compact":compact}),
    )
}

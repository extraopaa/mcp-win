use crate::{
    advanced, history,
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};
const TOTAL: u64 = 500 * 1024 * 1024;
const FILE: u64 = crate::limits::DOCUMENT_BYTES;
#[derive(Deserialize, schemars::JsonSchema)]
pub struct InspectArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ExtractArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub entries: Vec<String>,
    #[serde(default)]
    pub destination: String,
}
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateArgs {
    #[serde(default)]
    pub folder: String,
    pub paths: Vec<String>,
    pub output_path: String,
}
#[derive(Serialize, Deserialize)]
pub enum Command {
    Inspect {
        folder: String,
        path: String,
        offset: usize,
        limit: usize,
    },
    Extract {
        folder: String,
        path: String,
        entries: Vec<String>,
        destination: String,
    },
    Create {
        folder: String,
        paths: Vec<String>,
        output_path: String,
    },
}
#[derive(Clone, Serialize)]
struct Entry {
    path: String,
    bytes: u64,
    directory: bool,
    unsafe_entry: bool,
}
fn name(raw: &str) -> Result<String, String> {
    let name = raw.replace('\\', "/");
    let name = name.trim_end_matches('/');
    library::validate_relative(name)?;
    if name.is_empty()
        || name.len() > 240
        || Path::new(name).is_absolute()
        || name.split('/').any(|p| p.is_empty() || p == ".")
        || history::reserved(Path::new(name))
    {
        return Err("Chemin d’archive interdit".into());
    }
    Ok(name.into())
}
fn format(path: &Path) -> Result<&'static str, String> {
    let n = path.to_string_lossy().to_lowercase();
    if n.ends_with(".tar.gz") || n.ends_with(".tgz") {
        Ok("tgz")
    } else if n.ends_with(".tar") {
        Ok("tar")
    } else if n.ends_with(".zip") {
        Ok("zip")
    } else if n.ends_with(".7z") {
        Ok("7z")
    } else {
        Err("Formats : ZIP, TAR, TAR.GZ/TGZ et 7z".into())
    }
}
fn tar_reader(path: &Path) -> Result<Box<dyn Read>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if format(path)? == "tgz" {
        Ok(Box::new(flate2::read::GzDecoder::new(file)))
    } else {
        Ok(Box::new(file))
    }
}
fn inspect(path: &Path) -> Result<Vec<Entry>, String> {
    let mut result = Vec::new();
    let mut total = 0u64;
    let mut add = |raw: &str, size: u64, dir: bool, unsafe_type: bool| -> Result<(), String> {
        total = total.saturating_add(size);
        if total > TOTAL || result.len() >= 10000 {
            return Err("Archive limitée à 100 Mo décompressés et 10 000 entrées".into());
        }
        let normalized = name(raw);
        result.push(Entry {
            path: normalized
                .clone()
                .unwrap_or_else(|_| raw.chars().take(240).collect()),
            bytes: size,
            directory: dir,
            unsafe_entry: unsafe_type || normalized.is_err() || size > FILE,
        });
        Ok(())
    };
    match format(path)? {
        "zip" => {
            let mut zip = zip::ZipArchive::new(fs::File::open(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            if zip.len() > 10000 {
                return Err("Archive trop complexe".into());
            }
            for i in 0..zip.len() {
                let file = zip.by_index(i).map_err(|e| e.to_string())?;
                let mode = file.unix_mode().unwrap_or(0) & 0o170000;
                add(
                    file.name(),
                    file.size(),
                    file.is_dir(),
                    mode != 0 && mode != 0o100000 && mode != 0o040000,
                )?;
            }
        }
        "7z" => {
            let zip = sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty())
                .map_err(|e| e.to_string())?;
            for e in &zip.archive().files {
                let mode = (e.windows_attributes >> 16) & 0o170000;
                add(
                    e.name(),
                    e.size(),
                    e.is_directory(),
                    e.is_anti_item()
                        || e.windows_attributes & 0x400 != 0
                        || (mode != 0 && mode != 0o100000 && mode != 0o040000),
                )?;
            }
        }
        _ => {
            let mut tar = tar::Archive::new(tar_reader(path)?);
            for entry in tar.entries().map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let raw = entry
                    .path()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into_owned();
                let kind = entry.header().entry_type();
                add(
                    &raw,
                    entry.size(),
                    kind.is_dir(),
                    !kind.is_file() && !kind.is_dir(),
                )?;
            }
        }
    };
    Ok(result)
}
fn selected_bytes(
    path: &Path,
    wanted: &HashSet<String>,
) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut out = HashMap::new();
    let mut total = 0u64;
    let mut capture = |raw: &str, reader: &mut dyn Read| -> Result<(), String> {
        let normalized = name(raw).ok();
        let selected = normalized.as_ref().is_some_and(|n| wanted.contains(n));
        let mut data = Vec::new();
        let mut buf = [0u8; 16384];
        let mut size = 0u64;
        loop {
            let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            total = total.saturating_add(n as u64);
            size += n as u64;
            if total > TOTAL || (selected && size > FILE) {
                return Err("Décompression au-delà des limites autorisées".into());
            }
            if selected {
                data.extend_from_slice(&buf[..n]);
            }
        }
        if let Some(n) = normalized.filter(|_| selected) {
            if out.insert(n, data).is_some() {
                return Err("Entrée dupliquée".into());
            }
        }
        Ok(())
    };
    match format(path)? {
        "zip" => {
            let mut zip = zip::ZipArchive::new(fs::File::open(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            for i in 0..zip.len() {
                let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
                let raw = file.name().to_owned();
                if name(&raw).is_ok_and(|n| wanted.contains(&n)) {
                    capture(&raw, &mut file)?;
                }
            }
        }
        "7z" => {
            let mut zip = sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty())
                .map_err(|e| e.to_string())?;
            zip.for_each_entries(|entry, reader| {
                capture(entry.name(), reader).map_err(sevenz_rust::Error::other)?;
                Ok(true)
            })
            .map_err(|e| e.to_string())?;
        }
        _ => {
            let mut tar = tar::Archive::new(tar_reader(path)?);
            for entry in tar.entries().map_err(|e| e.to_string())? {
                let mut entry = entry.map_err(|e| e.to_string())?;
                let raw = entry
                    .path()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into_owned();
                capture(&raw, &mut entry)?;
            }
        }
    };
    Ok(out)
}
fn prepare_parent(folders: &[SharedFolder], folder: &str, relative: &str) -> Result<(), String> {
    let path = Path::new(relative);
    let mut at = PathBuf::new();
    for part in path.parent().unwrap_or(Path::new("")).components() {
        at.push(part);
        let root = library::resolve(folders, folder, "")?;
        let candidate = root.join(&at);
        if !candidate.exists() {
            fs::create_dir(&candidate).map_err(|e| e.to_string())?;
        }
        let meta = fs::symlink_metadata(&candidate).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return Err("Extraction via jonction interdite".into());
            }
        }
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("Parent d’extraction invalide".into());
        }
        library::resolve(folders, folder, &at.to_string_lossy())?;
    }
    Ok(())
}
pub(crate) fn execute(folders: &[SharedFolder], command: Command) -> Result<Value, String> {
    match command {
        Command::Inspect {
            folder,
            path,
            offset,
            limit,
        } => {
            let source = advanced::input(folders, &folder, &path)?;
            let entries = inspect(&source)?;
            let total = entries.len();
            let limit = limit.clamp(1, 500);
            let expanded = entries.iter().map(|e| e.bytes).sum::<u64>();
            Ok(
                json!({"entries":entries.into_iter().skip(offset).take(limit).collect::<Vec<_>>(),"total":total,"expanded_bytes":expanded,"next_offset":(offset.saturating_add(limit)<total).then_some(offset.saturating_add(limit))}),
            )
        }
        Command::Extract {
            folder,
            path,
            entries,
            destination,
        } => {
            let shared = library::select_folder(folders, &folder)?;
            if !shared.writable {
                return Err("Écriture désactivée pour ce dossier".into());
            }
            if entries.is_empty() || entries.len() > 20 {
                return Err("Sélectionner de 1 à 20 fichiers, sans extraction globale".into());
            }
            let destination = if destination.is_empty() {
                String::new()
            } else {
                name(&destination)?
            };
            let source = advanced::input(folders, &folder, &path)?;
            let metadata = inspect(&source)?;
            let mut selected = HashSet::new();
            let mut targets = HashMap::new();
            let mut case = HashSet::new();
            for selected_name in entries {
                let selected_name = name(&selected_name)?;
                let found: Vec<_> = metadata
                    .iter()
                    .filter(|e| e.path == selected_name)
                    .collect();
                if found.len() != 1 || found[0].unsafe_entry || found[0].directory {
                    return Err(format!(
                        "Entrée absente, ambiguë ou dangereuse : {selected_name}"
                    ));
                }
                let target = if destination.is_empty() {
                    selected_name.clone()
                } else {
                    format!("{destination}/{selected_name}")
                };
                if !case.insert(target.to_lowercase()) {
                    return Err("Sorties Windows ambiguës".into());
                }
                let root = library::resolve(folders, &folder, "")?;
                if root.join(&target).exists() {
                    return Err(format!(
                        "Destination déjà présente, aucun écrasement : {target}"
                    ));
                }
                selected.insert(selected_name.clone());
                targets.insert(selected_name, target);
            }
            let mut bytes = selected_bytes(&source, &selected)?;
            if bytes.len() != selected.len() {
                return Err("Sélection incomplète : aucune sortie créée".into());
            }
            let mut prepared = Vec::new();
            for (entry, target) in &targets {
                prepare_parent(folders, &folder, target)?;
                writing::destination(folders, &folder, target)?;
                prepared.push((entry.clone(), target.clone()));
            }
            prepared.sort();
            let mut written = Vec::new();
            for (entry, target) in prepared {
                match writing::commit(
                    folders,
                    &folder,
                    &target,
                    &bytes.remove(&entry).unwrap(),
                    None,
                ) {
                    Ok(v) => written.push(v),
                    Err(error) => {
                        return Ok(json!({"complete":false,"written":written,"error":error}));
                    }
                }
            }
            Ok(json!({"complete":true,"written":written}))
        }
        Command::Create {
            folder,
            paths,
            output_path,
        } => {
            writing::destination(folders, &folder, &output_path)?;
            if paths.is_empty() || paths.len() > 100 {
                return Err("Choisir 1 à 100 fichiers".into());
            }
            let mut total = 0u64;
            let mut files = Vec::new();
            let mut names = HashSet::new();
            for path in paths {
                let normalized = name(&path)?;
                if !names.insert(normalized.to_lowercase()) {
                    return Err("Chemins dupliqués".into());
                }
                let full = advanced::input(folders, &folder, &path)?;
                let len = fs::metadata(&full).map_err(|e| e.to_string())?.len();
                total += len;
                if total > TOTAL {
                    return Err("Sources limitées à 100 Mo".into());
                }
                files.push((normalized, full, len));
            }
            let cursor = Cursor::new(Vec::new());
            let bytes = match format(Path::new(&output_path))? {
                "zip" => {
                    let mut zip = zip::ZipWriter::new(cursor);
                    for (name, path, _) in &files {
                        zip.start_file(
                            name,
                            zip::write::SimpleFileOptions::default()
                                .compression_method(zip::CompressionMethod::Deflated),
                        )
                        .map_err(|e| e.to_string())?;
                        std::io::copy(
                            &mut fs::File::open(path).map_err(|e| e.to_string())?,
                            &mut zip,
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    zip.finish().map_err(|e| e.to_string())?.into_inner()
                }
                "7z" => {
                    let mut zip =
                        sevenz_rust::SevenZWriter::new(cursor).map_err(|e| e.to_string())?;
                    for (name, path, _) in &files {
                        let entry = sevenz_rust::SevenZArchiveEntry::from_path(path, name.clone());
                        zip.push_archive_entry(
                            entry,
                            Some(fs::File::open(path).map_err(|e| e.to_string())?),
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    zip.finish().map_err(|e| e.to_string())?.into_inner()
                }
                kind => {
                    let mut tar = tar::Builder::new(Vec::new());
                    for (name, path, size) in &files {
                        let mut header = tar::Header::new_gnu();
                        header.set_size(*size);
                        header.set_mode(0o644);
                        header.set_cksum();
                        tar.append_data(
                            &mut header,
                            name,
                            fs::File::open(path).map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    let raw = tar.into_inner().map_err(|e| e.to_string())?;
                    if kind == "tgz" {
                        let mut gz = flate2::write::GzEncoder::new(
                            Vec::new(),
                            flate2::Compression::default(),
                        );
                        gz.write_all(&raw).map_err(|e| e.to_string())?;
                        gz.finish().map_err(|e| e.to_string())?
                    } else {
                        raw
                    }
                }
            };
            let mut result = writing::commit(folders, &folder, &output_path, &bytes, None)?;
            result["entries"] = json!(files.len());
            Ok(result)
        }
    }
}

pub async fn run(folders: Vec<SharedFolder>, command: Command) -> Result<Value, String> {
    crate::worker::run(folders, crate::worker::Operation::Archive(command)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_formats_and_reject_archive_traversal() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("note.txt"), "Bonjour archive").unwrap();
        let folders = [SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: true,
        }];
        for ext in ["zip", "tar", "tar.gz", "7z"] {
            let out = format!("deliverable.{ext}");
            execute(
                &folders,
                Command::Create {
                    folder: "".into(),
                    paths: vec!["note.txt".into()],
                    output_path: out.clone(),
                },
            )
            .unwrap();
            let entries = inspect(&temp.path().join(&out)).unwrap();
            assert_eq!(entries[0].path, "note.txt");
            let found =
                selected_bytes(&temp.path().join(&out), &HashSet::from(["note.txt".into()]))
                    .unwrap();
            assert_eq!(found["note.txt"], b"Bonjour archive");
        }
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("../outside.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"bad").unwrap();
        fs::write(
            temp.path().join("evil.zip"),
            zip.finish().unwrap().into_inner(),
        )
        .unwrap();
        assert!(inspect(&temp.path().join("evil.zip")).unwrap()[0].unsafe_entry);
        assert!(
            execute(
                &folders,
                Command::Extract {
                    folder: "".into(),
                    path: "evil.zip".into(),
                    entries: vec!["../outside.txt".into()],
                    destination: "output".into()
                }
            )
            .is_err()
        );
        assert!(!temp.path().join("output").exists());
        execute(
            &folders,
            Command::Extract {
                folder: "".into(),
                path: "deliverable.zip".into(),
                entries: vec!["note.txt".into()],
                destination: "output".into(),
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join("output/note.txt")).unwrap(),
            "Bonjour archive"
        );
    }
}

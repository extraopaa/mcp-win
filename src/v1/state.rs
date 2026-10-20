use base64::Engine;
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

static ROOT: OnceLock<PathBuf> = OnceLock::new();
pub static LOCK: Mutex<()> = Mutex::new(());
pub fn initialize(directory: &Path) {
    let _ = ROOT.set(directory.join(".mcp-private"));
}
pub fn root() -> Result<PathBuf, String> {
    let path = ROOT.get().cloned().unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/qa-v1/.mcp-private")
    });
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let parent = path.parent().ok_or("Dossier privé absent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&temp, path).map_err(|e| e.to_string())
}
pub fn scope(path: &Path) -> String {
    crate::writing::hash(path.to_string_lossy().as_bytes())
}
pub fn valid_id(id: &str) -> Result<(), String> {
    if id.len() != 32 || !id.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Identifiant privé invalide".into());
    }
    Ok(())
}
pub fn reserve(directory: &Path, additional: u64, limit: u64) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }
    let mut bytes = 0u64;
    let mut count = 0;
    for entry in fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
    {
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        bytes = bytes.saturating_add(metadata.len());
        count += 1;
    }
    if bytes.saturating_add(additional) > limit || count > 4000 {
        return Err("Stockage privé plein pour cette fonction ; archiver les anciens résultats avant de continuer".into());
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct Brand {
    pub id: String,
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub font: String,
    pub organization: String,
    pub contact: String,
    pub footer: String,
    /// PNG/JPEG en base64 (sans préfixe), conservé localement. Maximum 2 Mio décodés.
    pub logo_base64: String,
}
impl Default for Brand {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "MCP Win — Professionnel".into(),
            primary: "6366F1".into(),
            secondary: "3B82F6".into(),
            font: "Lato".into(),
            organization: String::new(),
            contact: String::new(),
            footer: String::new(),
            logo_base64: String::new(),
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
pub struct Brands {
    pub selected: String,
    pub profiles: Vec<Brand>,
}
pub fn brands() -> Result<Brands, String> {
    let file = root()?.join("brands.json");
    if !file.exists() {
        return Ok(Brands {
            selected: "default".into(),
            profiles: vec![Brand::default()],
        });
    }
    let bytes = fs::read(file).map_err(|e| e.to_string())?;
    if bytes.len() > 12_000_000 {
        return Err("Profils trop volumineux".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
pub fn selected(id: Option<&str>) -> Result<Brand, String> {
    let b = brands()?;
    let id = id.unwrap_or(&b.selected);
    b.profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "Profil visuel inconnu ; consulter list_brand_profiles".into())
}
pub fn logo(brand: &Brand) -> Result<Option<Vec<u8>>, String> {
    if brand.logo_base64.is_empty() {
        return Ok(None);
    }
    if brand.logo_base64.len() > 3_000_000 {
        return Err("Logo limité à 2 Mio".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&brand.logo_base64)
        .map_err(|_| "Base64 du logo invalide")?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Logo limité à 2 Mio".into());
    }
    let format = image::guess_format(&bytes).map_err(|_| "Logo PNG/JPEG requis")?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return Err("Logo PNG/JPEG requis".into());
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(32 * 1024 * 1024);
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?;
    let mut bytes = crate::advanced::png(&image.thumbnail(1024, 1024))?;
    if bytes.len() > 2 * 1024 * 1024 {
        bytes = crate::advanced::png(&image.thumbnail(512, 512))?
    }
    Ok(Some(bytes))
}
pub fn save_brands(mut data: Brands) -> Result<(), String> {
    let _lock = LOCK.lock().map_err(|_| "Profils occupés")?;
    if data.profiles.is_empty() || data.profiles.len() > 10 {
        return Err("Entre 1 et 10 profils requis".into());
    }
    let mut ids = std::collections::HashSet::new();
    for p in &mut data.profiles {
        if p.id.is_empty() {
            p.id = uuid::Uuid::new_v4().simple().to_string()
        }
        if p.id.len() > 64
            || !p.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            || !ids.insert(p.id.clone())
        {
            return Err("Identifiant de profil invalide ou dupliqué".into());
        }
        for c in [&mut p.primary, &mut p.secondary] {
            *c = c.trim_start_matches('#').to_ascii_uppercase();
            if c.len() != 6 || !c.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("Couleur hexadécimale attendue".into());
            }
        }
        if !["Lato", "Libertinus Serif", "DejaVu Sans Mono"].contains(&p.font.as_str()) {
            return Err("Police intégrée : Lato, Libertinus Serif ou DejaVu Sans Mono".into());
        }
        if p.name.len() > 100
            || p.organization.len() > 200
            || p.contact.len() > 1000
            || p.footer.len() > 250
        {
            return Err("Texte du profil trop long".into());
        }
        if [&p.name, &p.organization, &p.contact, &p.footer]
            .iter()
            .any(|s| {
                s.chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\r'))
            })
        {
            return Err("Caractère de contrôle interdit".into());
        }
        logo(p)?;
    }
    if !ids.contains(&data.selected) {
        return Err("Choisir un profil par défaut existant".into());
    }
    if serde_json::to_vec(&data).map_err(|e| e.to_string())?.len() > 12_000_000 {
        return Err("Ensemble des profils limité à 12 Mo".into());
    }
    write_json(&root()?.join("brands.json"), &data)
}

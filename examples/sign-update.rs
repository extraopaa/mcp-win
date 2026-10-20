// Release utility: private key stays outside the repository and application.
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("generate") if args.len() == 4 => {
            let path = PathBuf::from(&args[2]);
            if path.exists() {
                return Err("Private key already exists".into());
            }
            let mut seed = [0u8; 32];
            getrandom::fill(&mut seed).map_err(|e| e.to_string())?;
            let key = SigningKey::from_bytes(&seed);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            use std::io::Write;
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?
                .write_all(STANDARD.encode(seed).as_bytes())?;
            fs::write(&args[3], STANDARD.encode(key.verifying_key().as_bytes()))?;
            println!("Signing key created; public key exported.");
        }
        Some("sign") if args.len() == 5 => {
            let seed: [u8; 32] = STANDARD
                .decode(fs::read_to_string(&args[2])?.trim())?
                .try_into()
                .map_err(|_| "Invalid key")?;
            let key = SigningKey::from_bytes(&seed);
            let public = fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/updater-public-key.txt"
            ))?;
            if STANDARD.encode(key.verifying_key().as_bytes()) != public.trim() {
                return Err("Signing key does not match embedded public key".into());
            }
            fs::write(
                &args[4],
                STANDARD.encode(key.sign(&fs::read(&args[3])?).to_bytes()),
            )?;
            println!("Manifest signed.");
        }
        _ => {
            return Err(
                "generate PRIVATE_KEY PUBLIC_KEY | sign PRIVATE_KEY MANIFEST SIGNATURE".into(),
            );
        }
    }
    Ok(())
}

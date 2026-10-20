//! Ouverture des liens de l'interface dans le navigateur Windows, hors de la WebView MCP.

fn destination(link: &str) -> Result<&'static str, String> {
    match link {
        "repository" => Ok("https://github.com/extraopaa/mcp-win"),
        "documentation" => Ok("https://github.com/extraopaa/mcp-win/blob/main/docs/wiki/Home.md"),
        "getting-started" => {
            Ok("https://github.com/extraopaa/mcp-win/blob/main/docs/wiki/Premiers-pas.md")
        }
        "connection-guide" => {
            Ok("https://github.com/extraopaa/mcp-win/blob/main/docs/wiki/Connexion-IA.md")
        }
        "tools-reference" => Ok("https://github.com/extraopaa/mcp-win/blob/main/docs/MCP-TOOLS.md"),
        _ => Err("Ce lien ne fait pas partie des pages de MCP Win.".into()),
    }
}

#[tauri::command]
pub fn open_external_link(link: String) -> Result<(), String> {
    let url = destination(&link)?;
    open_in_browser(url)
}

#[cfg(windows)]
fn open_in_browser(url: &str) -> Result<(), String> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};

    let operation: Vec<u16> = OsStr::new("open").encode_wide().chain(Some(0)).collect();
    let address: Vec<u16> = OsStr::new(url).encode_wide().chain(Some(0)).collect();
    // L'adresse provient exclusivement de la liste fixe ci-dessus. Aucun chemin,
    // argument de commande ni protocole fourni par le client n'est exécuté.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            address.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if result <= 32 {
        return Err(format!(
            "Impossible d’ouvrir votre navigateur (code Windows {result}). Adresse : {url}"
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn open_in_browser(_url: &str) -> Result<(), String> {
    Err("L’ouverture du navigateur est disponible dans l’application Windows.".into())
}

#[cfg(test)]
mod tests {
    use super::destination;

    #[test]
    fn external_links_only_allow_fixed_project_pages() {
        for link in [
            "repository",
            "documentation",
            "getting-started",
            "connection-guide",
            "tools-reference",
        ] {
            let url = reqwest::Url::parse(destination(link).unwrap()).unwrap();
            assert_eq!(url.scheme(), "https");
            assert_eq!(url.host_str(), Some("github.com"));
            assert!(url.path().starts_with("/extraopaa/mcp-win"));
            assert!(url.query().is_none());
        }
        for untrusted in [
            "",
            "https://example.com",
            "https://github.com/extraopaa/mcp-win",
            "file:///C:/Windows/System32/calc.exe",
            "ms-settings:",
            "repository --argument",
            "../repository",
        ] {
            assert!(destination(untrusted).is_err());
        }
    }
}

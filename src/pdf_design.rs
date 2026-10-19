use crate::{
    advanced::{self, Asset},
    library::SharedFolder,
    writing,
};
use serde_json::{Value, json};
use std::path::Path;
use typst_syntax::{
    LinkedNode,
    ast::{self, AstNode},
};

pub fn normalize(source: &str) -> (String, Vec<String>) {
    let trimmed = source.trim_start_matches('\u{feff}').trim();
    let mut notes = Vec::new();
    let mut source = source.trim_start_matches('\u{feff}').to_owned();
    if let Some((first, rest)) = trimmed.split_once('\n') {
        if ["```typst", "```typ", "```"].contains(&first.trim().to_ascii_lowercase().as_str())
            && rest.trim_end().ends_with("```")
        {
            source = rest
                .trim_end()
                .strip_suffix("```")
                .unwrap()
                .trim_end()
                .to_owned();
            notes.push("Balises Markdown externes retirées".into());
        }
    }
    let tree = typst_syntax::parse(&source);
    if !tree.errors_and_warnings().0.is_empty() {
        return (source, notes);
    }
    fn walk(node: LinkedNode<'_>, positions: &mut Vec<usize>) {
        if let Some(named) = node.get().cast::<ast::Named>() {
            let page = node
                .parent()
                .and_then(|n| n.parent())
                .is_some_and(|parent| {
                    let target = parent
                        .get()
                        .cast::<ast::SetRule>()
                        .map(|s| s.target())
                        .or_else(|| parent.get().cast::<ast::FuncCall>().map(|c| c.callee()));
                    matches!(target,Some(ast::Expr::Ident(id)) if id.as_str()=="page")
                });
            if page
                && ["header", "footer"].contains(&named.name().as_str())
                && !matches!(
                    named.expr(),
                    ast::Expr::Contextual(_) | ast::Expr::Closure(_)
                )
            {
                fn contextual(n: &typst_syntax::SyntaxNode) -> bool {
                    n.cast::<ast::Ident>().is_some_and(|id| {
                        ["counter", "here", "query", "state"].contains(&id.as_str())
                    }) || n.children().any(contextual)
                }
                if contextual(named.expr().to_untyped()) {
                    if let Some(expr) = node
                        .children()
                        .find(|child| std::ptr::eq(child.get(), named.expr().to_untyped()))
                    {
                        positions.push(expr.range().start);
                        return;
                    }
                }
            }
        }
        for child in node.children() {
            walk(child, positions);
        }
    }
    let mut positions = Vec::new();
    walk(LinkedNode::new(&tree), &mut positions);
    positions.sort_unstable();
    positions.dedup();
    for position in positions.iter().rev() {
        source.insert_str(*position, "context ");
    }
    if !positions.is_empty() {
        notes.push("Contexte de pagination ajouté aux en-têtes/pieds de page".into());
    }
    (source, notes)
}

pub fn diagnostics(raw: &str) -> Value {
    let mut hints = Vec::new();
    if raw.contains("context is known") {
        hints.push("Utiliser footer: context [...] ou #context counter(page).display().");
    }
    if raw.contains("unknown font family") {
        hints.push("Polices disponibles : Lato, Libertinus Serif, New Computer Modern, New Computer Modern Math, DejaVu Sans Mono. Fournir les autres polices en assets .ttf/.otf.");
    }
    if raw.contains("file not found") || raw.contains("failed to load file") {
        hints.push("Déclarer chaque image/police/fichier importé dans assets avec path (source partagée) et name (chemin utilisé dans Typst).");
    }
    if raw.contains("unexpected argument")
        || raw.contains("unknown variable")
        || raw.contains("expected")
    {
        hints.push("Source attendue : Typst 0.15.1, pas LaTeX/HTML/CSS. Lire get_authoring_guide ; vérifier noms des fonctions, arguments, virgules et délimiteurs.");
    }
    json!({"message":raw,"hints":hints,"typst_version":"0.15.1"})
}

#[cfg(test)]
pub async fn validate(
    folders: &[SharedFolder],
    folder: &str,
    source: &str,
    assets: &[Asset],
) -> Value {
    validate_with_data(folders, folder, source, assets, None).await
}
pub async fn validate_with_data(
    folders: &[SharedFolder],
    folder: &str,
    source: &str,
    assets: &[Asset],
    data: Option<&Value>,
) -> Value {
    match advanced::compile_typst_with_data(folders, folder, source, assets, data).await {
        Ok(pdf) => {
            let normalized_source = (pdf.source != source).then_some(pdf.source);
            let writable = crate::library::select_folder(folders, folder).is_ok_and(|f| f.writable);
            json!({"valid":true,"can_create":writable,"bytes":pdf.bytes.len(),"warnings":pdf.warnings,"adjustments":pdf.adjustments,"normalized_source":normalized_source,"available_fonts":["Lato","Libertinus Serif","New Computer Modern","New Computer Modern Math","DejaVu Sans Mono"],"next_action":if writable {"Créer avec create_designed_pdf puis utiliser le path retourné."} else {"Source valide mais dossier en lecture seule : choisir un dossier writable ou activer son écriture dans l'application."}})
        }
        Err(error) => {
            json!({"valid":false,"diagnostics":diagnostics(&error),"next_action":"Corriger source/assets puis rappeler validate_designed_pdf avant create_designed_pdf."})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_real_page_arguments_without_touching_literals_or_closures() {
        let source =
            "```typst\n#set page(footer: align(right, counter(page).display()))\nBonjour\n```";
        let (source, notes) = normalize(source);
        assert!(source.contains("footer: context align"), "{source}");
        assert_eq!(notes.len(), 2);
        let (again, notes) = normalize(&source);
        assert_eq!(again, source);
        assert!(notes.is_empty());
        for source in [
            "#let x = (footer: counter(page).display())",
            "#set page(footer: it => counter(page).display())",
            "`footer: counter(page).display()`",
        ] {
            assert_eq!(normalize(source).0, source);
        }
    }
    #[tokio::test]
    async fn preflight_reports_errors_and_fonts_without_writing_shared_files() {
        let temp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "qa".into(),
            path: temp.path().into(),
            writable: false,
        }];
        let report = validate(&folders, "", "#text(size: [", &[]).await;
        assert_eq!(report["valid"], false);
        assert!(report["diagnostics"]["hints"].is_array());
        let report = validate(
            &folders,
            "",
            "#set text(font: \"MissingFontForTest\")\nBonjour",
            &[],
        )
        .await;
        assert_eq!(report["valid"], true);
        assert_eq!(report["can_create"], false);
        assert!(
            report["warnings"]
                .as_str()
                .unwrap()
                .contains("unknown font family")
        );
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
    }
    #[tokio::test]
    async fn creates_fixed_context_and_versions_collisions_without_overwriting() {
        let temp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "qa".into(),
            path: temp.path().into(),
            writable: true,
        }];
        let source = "```typst\n#set page(footer: align(right, counter(page).display()))\n#set text(font: \"Lato\")\n*Bonjour* _été_\n```";
        let (first, second) = tokio::join!(
            create(&folders, "", "report", source, &[], "rename"),
            create(&folders, "", "report", source, &[], "rename")
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first["path"], second["path"]);
        assert!(temp.path().join("report.pdf").is_file());
        assert!(temp.path().join("report_2.pdf").is_file());
        let original = std::fs::read(temp.path().join("report.pdf")).unwrap();
        assert!(
            create(&folders, "", "report.pdf", source, &[], "error")
                .await
                .is_err()
        );
        assert_eq!(
            original,
            std::fs::read(temp.path().join("report.pdf")).unwrap()
        );
        assert_eq!(first["adjustments"].as_array().unwrap().len(), 2);
        let bad = [Asset {
            path: "missing".into(),
            name: "Main.Typ".into(),
        }];
        assert!(
            advanced::compile_typst(&folders, "", "Bonjour", &bad)
                .await
                .err()
                .unwrap()
                .contains("réservé")
        );
    }
}

#[cfg(test)]
pub async fn create(
    folders: &[SharedFolder],
    folder: &str,
    output: &str,
    source: &str,
    assets: &[Asset],
    collision_policy: &str,
) -> Result<Value, String> {
    create_with_data(
        folders,
        folder,
        output,
        source,
        assets,
        collision_policy,
        None,
    )
    .await
}
pub async fn create_with_data(
    folders: &[SharedFolder],
    folder: &str,
    output: &str,
    source: &str,
    assets: &[Asset],
    collision_policy: &str,
    data: Option<&Value>,
) -> Result<Value, String> {
    let requested_output = output;
    if output.trim().is_empty() || output.ends_with(['/', '\\']) {
        return Err("Nom de fichier PDF requis, par exemple rapport.pdf".into());
    }
    if !["rename", "error"].contains(&collision_policy) {
        return Err("collision_policy : rename ou error".into());
    }
    let path = Path::new(output);
    let output = if path.extension().is_none() {
        format!("{output}.pdf")
    } else {
        output.to_owned()
    };
    if !output.to_lowercase().ends_with(".pdf") {
        return Err("La sortie doit être un PDF (.pdf)".into());
    }
    let original = writing::destination(folders, folder, &output)?;
    if original.exists() && collision_policy == "error" {
        return Err("Ce fichier existe déjà. Choisir un nouveau nom ou collision_policy=rename pour conserver les deux versions.".into());
    }
    let pdf = advanced::compile_typst_with_data(folders, folder, source, assets, data)
        .await
        .map_err(|e| diagnostics(&e).to_string())?;
    for version in 1..=100 {
        let candidate = if version == 1 {
            output.clone()
        } else {
            let p = Path::new(&output);
            p.with_file_name(format!(
                "{}_{version}.pdf",
                p.file_stem().unwrap().to_string_lossy()
            ))
            .to_string_lossy()
            .into_owned()
        };
        if writing::destination(folders, folder, &candidate)?.exists() {
            if collision_policy == "error" {
                return Err(
                    "Le fichier vient d'être créé par une autre requête ; choisir un nouveau nom"
                        .into(),
                );
            }
            continue;
        }
        match writing::commit(folders, folder, &candidate, &pdf.bytes, None) {
            Ok(mut result) => {
                result["requested_path"] = json!(requested_output);
                result["extension_added"] = json!(output != requested_output);
                result["renamed"] = json!(candidate != output);
                result["warnings"] = json!(pdf.warnings);
                result["adjustments"] = json!(pdf.adjustments);
                result["next_action"] =
                    json!("Utiliser le path retourné pour render_pdf_page et read_file_bytes.");
                return Ok(result);
            }
            Err(_)
                if collision_policy == "rename"
                    && writing::destination(folders, folder, &candidate)?.exists() =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
    Err("100 noms de sortie déjà occupés : choisir un autre nom de document".into())
}

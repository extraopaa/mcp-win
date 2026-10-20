use super::*;
use crate::{library::SharedFolder, writing};
use serde_json::json;
use std::fs;
use tokio_util::sync::CancellationToken;
fn folders(path: &std::path::Path) -> Vec<SharedFolder> {
    vec![SharedFolder {
        id: uuid::Uuid::new_v4().simple().to_string(),
        path: path.into(),
        writable: true,
    }]
}
#[test]
fn larger_files_are_read_in_bounded_responses_and_oversized_inputs_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let bytes = vec![b'A'; 26 * 1024 * 1024];
    writing::commit(&scope, "", "large.txt", &bytes, None).unwrap();
    let read = crate::library::read(&scope, "", "large.txt", 1, 0, 32).unwrap();
    assert_eq!(read["text"].as_str().unwrap().len(), 32);
    let too_big = dir.path().join("too-big.pdf");
    fs::File::create(&too_big)
        .unwrap()
        .set_len(crate::limits::DOCUMENT_BYTES + 1)
        .unwrap();
    assert!(crate::advanced::input(&scope, "", "too-big.pdf").is_err());
}
#[test]
fn studio_accepts_larger_logo_and_normalizes_it() {
    use base64::Engine;
    let mut seed = 19u32;
    let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(1200, 1200, |_, _| {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        image::Rgba([
            (seed >> 24) as u8,
            (seed >> 16) as u8,
            (seed >> 8) as u8,
            255,
        ])
    }));
    let bytes = crate::advanced::png(&image).unwrap();
    assert!(bytes.len() > 2 * 1024 * 1024);
    let qa = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/qa-v101");
    fs::create_dir_all(&qa).unwrap();
    fs::write(qa.join("large-logo.png"), &bytes).unwrap();
    let brand = state::Brand {
        logo_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        ..Default::default()
    };
    let normalized = state::logo(&brand).unwrap().unwrap();
    assert!(normalized.len() <= 2 * 1024 * 1024);
    assert!(image::load_from_memory(&normalized).unwrap().width() <= 2048);
}
#[tokio::test]
async fn custom_brand_controls_word_and_pdf_page_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let args = design::Args {
        path: "custom.docx".into(),
        title: "Custom layout".into(),
        internal_brand: Some(state::Brand {
            body_size_pt: 14,
            margin_mm: 24,
            page_size: "us-letter".into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    design::create(&scope, args.clone()).await.unwrap();
    let parts = design::unpack(&fs::read(dir.path().join("custom.docx")).unwrap()).unwrap();
    let document = String::from_utf8(parts["word/document.xml"].clone()).unwrap();
    let styles = String::from_utf8(parts["word/styles.xml"].clone()).unwrap();
    assert!(document.contains("w:w=\"12240\" w:h=\"15840\""));
    assert!(document.contains("w:top=\"1361\""));
    assert!(styles.contains("w:sz w:val=\"28\""));
    design::create(
        &scope,
        design::Args {
            path: "custom.pdf".into(),
            ..args
        },
    )
    .await
    .unwrap();
    let (image, _) = crate::advanced::pdf_image(&scope, "", "custom.pdf", 1, 1000).unwrap();
    assert!((image.height() as f64 / image.width() as f64 - 11.0 / 8.5).abs() < 0.01);
}
#[tokio::test]
async fn spreadsheet_revision_preserves_types_and_refuses_stale_source() {
    use crate::generate::{CellEdit, CellKind};
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let result = crate::generate::spreadsheet(
        &scope,
        "",
        "budget.xlsx",
        None,
        None,
        &[
            CellEdit {
                sheet: "Budget".into(),
                cell: "A1".into(),
                kind: CellKind::Text,
                value: "Montant".into(),
            },
            CellEdit {
                sheet: "Budget".into(),
                cell: "A2".into(),
                kind: CellKind::Number,
                value: "12.5".into(),
            },
            CellEdit {
                sheet: "Budget".into(),
                cell: "B2".into(),
                kind: CellKind::Text,
                value: "Texte".into(),
            },
        ],
    )
    .unwrap();
    let bytes = fs::read(dir.path().join("budget.xlsx")).unwrap();
    let proposal = edits::propose(
        &scope,
        edits::EditArgs {
            folder: String::new(),
            path: "budget.xlsx".into(),
            output_path: "budget.xlsx".into(),
            expected_sha256: result["sha256"].as_str().unwrap().into(),
            label: None,
            edits: vec![
                edits::Edit {
                    id: "Budget!A2".into(),
                    text: "42.75".into(),
                    find: None,
                    replacement: None,
                },
                edits::Edit {
                    id: "Budget!B2".into(),
                    text: "=1+1".into(),
                    find: None,
                    replacement: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    let id = proposal["revision_id"].as_str().unwrap();
    fs::write(dir.path().join("budget.xlsx"), b"changed").unwrap();
    assert!(edits::apply(&scope, id, "apply").is_err());
    fs::write(dir.path().join("budget.xlsx"), &bytes).unwrap();
    edits::apply(&scope, id, "apply").unwrap();
    let book = umya_spreadsheet::reader::xlsx::read(dir.path().join("budget.xlsx")).unwrap();
    let sheet = book.sheet_by_name("Budget").unwrap();
    assert_eq!(sheet.cell("A2").unwrap().value(), "42.75");
    assert_eq!(sheet.cell("A2").unwrap().data_type(), "n");
    assert_eq!(sheet.cell("B2").unwrap().value(), "=1+1");
    assert!(sheet.cell("B2").unwrap().formula().is_empty());
    // Simulate interruption after file commit, before final revision status.
    let manifest = state::root()
        .unwrap()
        .join("revisions")
        .join(format!("{id}.json"));
    let mut revision: edits::Revision =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    revision.status = "pending".into();
    revision.applying = true;
    state::write_json(&manifest, &revision).unwrap();
    let recovered = edits::apply(&scope, id, "apply").unwrap();
    assert_eq!(recovered["recovered"], true);
    edits::apply(&scope, id, "undo").unwrap();
    assert_eq!(fs::read(dir.path().join("budget.xlsx")).unwrap(), bytes);
}
#[tokio::test]
async fn professional_documents_default_quality_models_and_targeted_review() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let args=design::Args{path:"rapport.pdf".into(),title:"Une IA au service des documents".into(),blocks:vec![
  design::Block{id:"intro".into(),kind:"heading".into(),text:"Des documents utiles et soignés".into(),level:1,..Default::default()},
  design::Block{id:"body".into(),kind:"paragraph".into(),text:"La mise en page professionnelle est activée par défaut. Les accents français, les données et les sources restent lisibles. #eval(\"code interdit\") est du texte, jamais du code.".into(),..Default::default()},
  design::Block{id:"table".into(),kind:"table".into(),columns:vec!["Usage".into(),"Résultat".into()],rows:vec![vec!["Analyse".into(),"Sources vérifiées".into()],vec!["Création".into(),"PDF et Office mis en forme".into()]],..Default::default()},
  design::Block{id:"note".into(),kind:"callout".into(),text:"Une validation visuelle complète le contrôle automatique.".into(),..Default::default()},
 ],..Default::default()};
    let pdf = design::create(&scope, args.clone()).await.unwrap();
    assert_eq!(pdf["design"]["mode"], "professional");
    assert_eq!(pdf["quality"]["passed"], true);
    let qa = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/qa-v1");
    fs::create_dir_all(&qa).unwrap();
    fs::copy(
        dir.path().join("rapport.pdf"),
        qa.join("rapport-professionnel.pdf"),
    )
    .unwrap();
    let (image, _) = crate::advanced::pdf_image(&scope, "", "rapport.pdf", 1, 1600).unwrap();
    fs::write(
        qa.join("rapport-professionnel.png"),
        crate::advanced::png(&image).unwrap(),
    )
    .unwrap();
    let inspected = edits::inspect(
        &scope,
        edits::InspectArgs {
            folder: String::new(),
            path: "rapport.pdf".into(),
            offset: None,
            limit: None,
        },
    )
    .unwrap();
    assert!(
        inspected["elements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["id"] == "table.row-1.cell-2")
    );
    let proposal = edits::propose(
        &scope,
        edits::EditArgs {
            folder: String::new(),
            path: "rapport.pdf".into(),
            output_path: "rapport-corrige.pdf".into(),
            expected_sha256: pdf["sha256"].as_str().unwrap().into(),
            edits: vec![edits::Edit {
                id: "table.row-1.cell-2".into(),
                text: "Références précises".into(),
                find: None,
                replacement: None,
            }],
            label: None,
        },
    )
    .await
    .unwrap();
    assert!(!dir.path().join("rapport-corrige.pdf").exists());
    let id = proposal["revision_id"].as_str().unwrap();
    let preview = edits::preview(&scope, id).unwrap();
    assert!(preview["images"].as_array().unwrap().len() >= 2);
    edits::apply(&scope, id, "apply").unwrap();
    assert!(dir.path().join("rapport-corrige.pdf").exists());
    edits::apply(&scope, id, "undo").unwrap();
    assert!(!dir.path().join("rapport-corrige.pdf").exists());
    let mut word = args.clone();
    word.path = "rapport.docx".into();
    let result = design::create(&scope, word).await.unwrap();
    assert_eq!(result["quality"]["passed"], true);
    let parts = design::unpack(&fs::read(dir.path().join("rapport.docx")).unwrap()).unwrap();
    assert!(
        String::from_utf8(parts["word/styles.xml"].clone())
            .unwrap()
            .contains("keepNext")
    );
    assert!(parts.contains_key("word/footer1.xml"));
    fs::copy(
        dir.path().join("rapport.docx"),
        qa.join("rapport-professionnel.docx"),
    )
    .unwrap();
    let mut deck = args;
    deck.path = "presentation.pptx".into();
    design::create(&scope, deck).await.unwrap();
}
#[test]
fn persistent_index_invalidates_modified_sources_and_keeps_cache_private() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    fs::write(
        dir.path().join("facture.txt"),
        "Facture démonstration\nTotal TTC : 120 euros",
    )
    .unwrap();
    let build = index::build(
        &scope,
        index::IndexArgs {
            folder: String::new(),
            offset: None,
            max_files: None,
        },
    )
    .unwrap();
    assert_eq!(build["indexed"], 1);
    let results = index::query(
        &scope,
        index::QueryArgs {
            folder: String::new(),
            query: "facture".into(),
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(results["matches"].as_array().unwrap().len(), 1);
    assert_eq!(results["matches"][0]["source_verified"], true);
    fs::write(
        dir.path().join("facture.txt"),
        "Un contenu complètement modifié",
    )
    .unwrap();
    let results = index::query(
        &scope,
        index::QueryArgs {
            folder: String::new(),
            query: "facture".into(),
            limit: None,
        },
    )
    .unwrap();
    assert!(results["matches"].as_array().unwrap().is_empty());
    assert_eq!(results["refresh_required"], true);
    fs::create_dir(dir.path().join(".mcp-private")).unwrap();
    fs::write(dir.path().join(".mcp-private/secret.txt"), "secret").unwrap();
    let files = crate::documents::paths(&scope, "", "", true, None)
        .unwrap()
        .0;
    assert!(!files.iter().any(|(p, _)| p.contains("secret")));
}
#[tokio::test]
async fn imported_office_edits_preserve_styles_and_restore_source() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let bytes = crate::generate::docx(
        "Titre",
        &[crate::generate::Section {
            heading: "Prix".into(),
            text: "Le prix est de 100 euros.".into(),
            bullets: vec![],
        }],
    )
    .unwrap();
    fs::write(dir.path().join("source.docx"), &bytes).unwrap();
    let inspected = edits::inspect(
        &scope,
        edits::InspectArgs {
            folder: String::new(),
            path: "source.docx".into(),
            offset: None,
            limit: None,
        },
    )
    .unwrap();
    let element = inspected["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["text"].as_str().unwrap().contains("100 euros"))
        .unwrap();
    let proposal = edits::propose(
        &scope,
        edits::EditArgs {
            folder: String::new(),
            path: "source.docx".into(),
            output_path: "source.docx".into(),
            expected_sha256: writing::hash(&bytes),
            edits: vec![edits::Edit {
                id: element["id"].as_str().unwrap().into(),
                text: String::new(),
                find: Some("100 euros".into()),
                replacement: Some("125 euros".into()),
            }],
            label: None,
        },
    )
    .await
    .unwrap();
    let id = proposal["revision_id"].as_str().unwrap();
    edits::apply(&scope, id, "apply").unwrap();
    assert!(
        crate::office::read(&dir.path().join("source.docx"), 1)
            .unwrap()
            .0
            .contains("125 euros")
    );
    edits::apply(&scope, id, "undo").unwrap();
    assert_eq!(fs::read(dir.path().join("source.docx")).unwrap(), bytes);
}
#[test]
fn structured_ocr_tables_keep_low_confidence_and_raw_values() {
    let mut words = vec![];
    for (r, values) in [["Article", "Prix"], ["Livre", "12,50"], ["Stylo", "2,00"]]
        .iter()
        .enumerate()
    {
        for (c, text) in values.iter().enumerate() {
            words.push(json!({"text":text,"confidence":if r==2{55.0}else{95.0},"box":{"x":10+c*200,"y":10+r*40,"width":70,"height":20}}));
        }
    }
    let result = ocr_layout::structure(&words);
    assert_eq!(result["tables"].as_array().unwrap().len(), 1);
    assert_eq!(result["tables"][0]["needs_review"], true);
    assert_eq!(result["tables"][0]["rows"][0][1], "12,50");
}
#[tokio::test]
async fn durable_workflow_references_group_undo_and_cancellation() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let job = jobs::start(
        scope.clone(),
        jobs::StartArgs {
            name: "Livrables".into(),
            folder: String::new(),
            steps: vec![
                jobs::Step {
                    id: "note".into(),
                    tool: "write_text_file".into(),
                    args: json!({"path":"note.txt","content":"Texte à conserver"}),
                },
                jobs::Step {
                    id: "zip".into(),
                    tool: "create_archive".into(),
                    args: json!({"paths":[{"$ref":"note.path"}],"output_path":"livrable.zip"}),
                },
            ],
        },
        CancellationToken::new(),
    )
    .unwrap();
    let id = job["id"].as_str().unwrap();
    for _ in 0..100 {
        let s = jobs::status(
            &scope,
            jobs::StatusArgs {
                id: id.into(),
                include_results: Some(true),
            },
        )
        .unwrap();
        if s["status"] != "running" {
            assert_eq!(s["status"], "completed", "{s}");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    assert!(dir.path().join("livrable.zip").exists());
    fs::write(dir.path().join("note.txt"), "Modification ultérieure").unwrap();
    assert!(jobs::undo(&scope, id).is_err());
    assert!(dir.path().join("livrable.zip").exists());
    fs::write(dir.path().join("note.txt"), "Texte à conserver").unwrap();
    jobs::undo(&scope, id).unwrap();
    assert!(!dir.path().join("note.txt").exists());
    assert!(!dir.path().join("livrable.zip").exists());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(
        jobs::start(
            scope,
            jobs::StartArgs {
                name: "Refus".into(),
                folder: String::new(),
                steps: vec![jobs::Step {
                    id: "x".into(),
                    tool: "write_text_file".into(),
                    args: json!({"path":"non.txt","content":"Non"})
                }]
            },
            cancelled
        )
        .is_err()
    );
    assert!(!dir.path().join("non.txt").exists());
}
#[tokio::test]
async fn workflow_resume_does_not_replay_completed_writes() {
    let dir = tempfile::tempdir().unwrap();
    let scope = folders(dir.path());
    let job = jobs::start(
        scope.clone(),
        jobs::StartArgs {
            name: "Reprise".into(),
            folder: String::new(),
            steps: vec![
                jobs::Step {
                    id: "write".into(),
                    tool: "write_text_file".into(),
                    args: json!({"path":"saved.txt","content":"Durable"}),
                },
                jobs::Step {
                    id: "read".into(),
                    tool: "read_file".into(),
                    args: json!({"path":"missing.txt"}),
                },
            ],
        },
        CancellationToken::new(),
    )
    .unwrap();
    let id = job["id"].as_str().unwrap();
    for _ in 0..100 {
        let s = jobs::status(
            &scope,
            jobs::StatusArgs {
                id: id.into(),
                include_results: None,
            },
        )
        .unwrap();
        if s["status"] != "running" {
            assert_eq!(s["status"], "failed");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    fs::write(dir.path().join("missing.txt"), "Disponible").unwrap();
    jobs::control(
        scope.clone(),
        jobs::ControlArgs {
            id: id.into(),
            action: "resume".into(),
        },
        CancellationToken::new(),
    )
    .unwrap();
    let mut completed = false;
    for _ in 0..100 {
        let s = jobs::status(
            &scope,
            jobs::StatusArgs {
                id: id.into(),
                include_results: None,
            },
        )
        .unwrap();
        if s["status"] != "running" {
            assert_eq!(s["status"], "completed", "{s}");
            completed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    }
    assert!(completed);
    assert_eq!(
        fs::read_to_string(dir.path().join("saved.txt")).unwrap(),
        "Durable"
    );
}

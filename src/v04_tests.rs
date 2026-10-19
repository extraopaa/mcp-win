use crate::{documents, generate, history, library::SharedFolder, patching, templates, writing};
use serde_json::json;
use std::fs;
fn folders(root: &std::path::Path) -> Vec<SharedFolder> {
    vec![SharedFolder {
        id: "qa".into(),
        path: root.into(),
        writable: true,
    }]
}
#[test]
fn recursive_search_filters_pagination_and_late_excel_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let f = folders(tmp.path());
    fs::create_dir(tmp.path().join("2024")).unwrap();
    fs::write(
        tmp.path().join("2024/notes.md"),
        "# Rapport\nFacture été\nTotal TTC 42\n## Résultats\nFacture finale\n",
    )
    .unwrap();
    let listed = documents::listing(&f, "", "", true, Some("**/*.md"), true, 0, 5).unwrap();
    assert_eq!(listed["entries"][0], "2024/notes.md");
    let first = documents::search(
        &f,
        documents::SearchArgs {
            folder: "".into(),
            query: "facture".into(),
            use_regex: None,
            case_sensitive: None,
            glob_pattern: None,
            max_matches: Some(1),
            max_files: None,
            cursor: None,
        },
    )
    .unwrap();
    assert_eq!(first["matches"][0]["line"], 2);
    let next = documents::search(
        &f,
        documents::SearchArgs {
            folder: "".into(),
            query: "facture".into(),
            use_regex: None,
            case_sensitive: None,
            glob_pattern: None,
            max_matches: Some(1),
            max_files: None,
            cursor: first["next_cursor"].as_str().map(str::to_owned),
        },
    )
    .unwrap();
    assert_eq!(next["matches"][0]["line"], 5);
    let filtered = documents::filtered_read(
        &f,
        "",
        "2024/notes.md",
        1,
        Some([2, 4]),
        Some("Total"),
        0,
        100,
    )
    .unwrap();
    assert_eq!(filtered["text"], "3: Total TTC 42");
    assert_eq!(
        documents::outline(&f, "", "2024/notes.md").unwrap()["headings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    generate::spreadsheet(
        &f,
        "",
        "late.xlsx",
        None,
        None,
        &[generate::CellEdit {
            sheet: "Data".into(),
            cell: "A6001".into(),
            kind: generate::CellKind::Text,
            value: "NEEDLE-AFTER-5000".into(),
        }],
    )
    .unwrap();
    let hits = documents::search(
        &f,
        documents::SearchArgs {
            folder: "".into(),
            query: "NEEDLE-AFTER-5000".into(),
            use_regex: None,
            case_sensitive: None,
            glob_pattern: Some("**/*.xlsx".into()),
            max_matches: None,
            max_files: None,
            cursor: None,
        },
    )
    .unwrap();
    assert_eq!(hits["matches"][0]["path"], "late.xlsx");
    assert_eq!(hits["matches"][0]["line"], 6001);
}
#[test]
fn compact_csv_patch_history_restore_and_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let f = folders(tmp.path());
    fs::write(
        tmp.path().join("data.csv"),
        "name;price\nAlpha;12\nBeta;24\nGamma;36\nDelta;48\n",
    )
    .unwrap();
    let compact = documents::compact_table(&f, "", "data.csv", 1).unwrap();
    assert_eq!(compact["columns"], json!(["name", "price"]));
    assert_eq!(compact["data_rows"], 4);
    assert_eq!(compact["sample_rows"].as_array().unwrap().len(), 3);
    let original = "Hello\nworld\n";
    writing::text(&f, "", "note.txt", original, None).unwrap();
    let patch = diffy::create_patch(original, "Hello\nRust\n").to_string();
    let modified = patching::apply(
        &f,
        patching::PatchArgs {
            folder: "".into(),
            path: "note.txt".into(),
            expected_sha256: writing::hash(original.as_bytes()),
            patch,
            part: None,
            output_path: None,
        },
    )
    .unwrap();
    let version = modified["previous_version"].as_str().unwrap();
    assert_eq!(history::list(&f, "", None).unwrap()["total"], 1);
    assert!(crate::library::resolve(&f, "", ".mcp-history").is_err());
    history::restore_local(&f, "", version).unwrap();
    assert_eq!(
        fs::read_to_string(tmp.path().join("note.txt")).unwrap(),
        original
    );
    assert_eq!(history::list(&f, "", None).unwrap()["total"], 2);
    assert!(history::restore(&f, "", version, "wrong-hash").is_err());
}
#[tokio::test]
async fn typst_json_and_word_split_run_templates_preserve_layout() {
    let tmp = tempfile::tempdir().unwrap();
    let f = folders(tmp.path());
    fs::create_dir(tmp.path().join("templates")).unwrap();
    fs::write(
        tmp.path().join("templates/report.typ"),
        "#set text(font: \"Lato\")\n= #data.title\n#data.body",
    )
    .unwrap();
    templates::create(
        f.clone(),
        templates::TemplateArgs {
            folder: "".into(),
            template: "templates/report.typ".into(),
            output_path: "templated.pdf".into(),
            data: json!({"title":"Rapport Été","body":"Une valeur sûre : #ne-pas-executer(42)"}),
            assets: vec![],
            collision_policy: None,
        },
    )
    .await
    .unwrap();
    let text = crate::library::read(&f, "", "templated.pdf", 1, 0, 8000).unwrap();
    assert!(
        text["text"]
            .as_str()
            .unwrap()
            .contains("#ne-pas-executer(42)")
    );
    let source = generate::docx("{{client}}", &[]).unwrap();
    let mut zip = crate::office::archive(&source).unwrap();
    use std::io::{Cursor, Read, Write};
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).unwrap();
        let name = file.name().to_owned();
        if name == "word/document.xml" {
            let mut xml = String::new();
            file.read_to_string(&mut xml).unwrap();
            let xml = xml.replace("{{client}}", "{{cli</w:t></w:r><w:r><w:t>ent}}");
            out.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            out.write_all(xml.as_bytes()).unwrap();
        } else {
            out.raw_copy_file(file).unwrap();
        }
    }
    let bytes = out.finish().unwrap().into_inner();
    fs::write(tmp.path().join("templates/brand.dotx"), &bytes).unwrap();
    let report = templates::inspect(&f, "", "templates/brand.dotx").unwrap();
    assert_eq!(report["placeholders"][0], "client");
    templates::office(
        &f,
        "",
        "templates/brand.dotx",
        "brand.docx",
        &json!({"client":"Élodie & Fils"}),
    )
    .unwrap();
    let text = crate::library::read(&f, "", "brand.docx", 1, 0, 8000).unwrap();
    assert!(text["text"].as_str().unwrap().contains("Élodie & Fils"));
    assert_eq!(
        fs::read(tmp.path().join("templates/brand.dotx")).unwrap(),
        bytes
    );
}

#[test]
fn merges_and_splits_pdf_without_replacing_originals() {
    let tmp = tempfile::tempdir().unwrap();
    let f = folders(tmp.path());
    generate::document(&f, "", "first.pdf", "First source", &[]).unwrap();
    generate::document(&f, "", "second.pdf", "Second source", &[]).unwrap();
    let source = fs::read(tmp.path().join("first.pdf")).unwrap();
    let merged = crate::pdf_ops::combine(
        &f,
        "",
        &[
            crate::pdf_ops::PdfInput {
                path: "first.pdf".into(),
                pages: None,
            },
            crate::pdf_ops::PdfInput {
                path: "second.pdf".into(),
                pages: None,
            },
        ],
        "merged.pdf",
        false,
    )
    .unwrap();
    assert_eq!(merged["pages"], 2);
    crate::pdf_ops::combine(
        &f,
        "",
        &[crate::pdf_ops::PdfInput {
            path: "merged.pdf".into(),
            pages: Some("2".into()),
        }],
        "selected.pdf",
        false,
    )
    .unwrap();
    let text = crate::library::read(&f, "", "selected.pdf", 1, 0, 8000).unwrap();
    assert!(text["text"].as_str().unwrap().contains("Second source"));
    assert!(!text["text"].as_str().unwrap().contains("First source"));
    assert_eq!(fs::read(tmp.path().join("first.pdf")).unwrap(), source);
    assert!(
        crate::pdf_ops::combine(
            &f,
            "",
            &[crate::pdf_ops::PdfInput {
                path: "first.pdf".into(),
                pages: Some("0-20".into())
            }],
            "bad.pdf",
            false
        )
        .is_err()
    );
    assert!(!tmp.path().join("bad.pdf").exists());
}

#[test]
fn patches_one_office_part_and_keeps_the_original() {
    let tmp = tempfile::tempdir().unwrap();
    let f = folders(tmp.path());
    let original = generate::docx("Before title", &[]).unwrap();
    fs::write(tmp.path().join("source.docx"), &original).unwrap();
    let part =
        crate::advanced::inspect_package(&f, "", "source.docx", Some("word/document.xml")).unwrap();
    let text = part["text"].as_str().unwrap();
    let patch = diffy::create_patch(text, &text.replace("Before title", "After title")).to_string();
    patching::apply(
        &f,
        patching::PatchArgs {
            folder: "".into(),
            path: "source.docx".into(),
            expected_sha256: writing::hash(&original),
            patch,
            part: Some("word/document.xml".into()),
            output_path: Some("modified.docx".into()),
        },
    )
    .unwrap();
    assert_eq!(fs::read(tmp.path().join("source.docx")).unwrap(), original);
    assert!(
        crate::library::read(&f, "", "modified.docx", 1, 0, 8000).unwrap()["text"]
            .as_str()
            .unwrap()
            .contains("After title")
    );
}

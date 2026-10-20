use crate::{
    library::{self, SharedFolder},
    office, writing,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[cfg(test)]
use std::path::Path;
use std::{fs, io::Cursor};

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Section {
    #[serde(default)]
    pub heading: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub bullets: Vec<String>,
}

#[cfg(test)]
pub fn document(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    title: &str,
    sections: &[Section],
) -> Result<Value, String> {
    writing::destination(folders, folder, path)?;
    if title.len() > 500
        || sections.len() > 100
        || sections
            .iter()
            .map(|s| {
                s.heading.len() + s.text.len() + s.bullets.iter().map(String::len).sum::<usize>()
            })
            .sum::<usize>()
            > 200_000
    {
        return Err(
            "Document limité à 100 sections, 200 000 octets de texte et un titre de 500 octets"
                .into(),
        );
    }
    let ext = Path::new(path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let invalid_control = |text: &str| {
        text.chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    };
    if invalid_control(title)
        || sections.iter().any(|s| {
            invalid_control(&s.heading)
                || invalid_control(&s.text)
                || s.bullets.iter().any(|b| invalid_control(b))
        })
    {
        return Err("Le document contient un caractère de contrôle non autorisé".into());
    }
    let bytes =
        match ext.as_str() {
            "pdf" => pdf(title, sections)?,
            "docx" => docx(title, sections)?,
            "pptx" => pptx(title, sections)?,
            _ => return Err(
                "Extension attendue : .pdf, .docx ou .pptx. Pour Excel, utiliser write_spreadsheet"
                    .into(),
            ),
        };
    writing::commit(folders, folder, path, &bytes, None)
}

pub(crate) fn docx(title: &str, sections: &[Section]) -> Result<Vec<u8>, String> {
    use docx_rs::{Docx, Paragraph, Run};
    let mut doc = Docx::new().add_paragraph(
        Paragraph::new()
            .style("Title")
            .add_run(Run::new().bold().size(40).add_text(title)),
    );
    for section in sections {
        if !section.heading.is_empty() {
            doc = doc.add_paragraph(
                Paragraph::new()
                    .style("Heading1")
                    .add_run(Run::new().bold().size(28).add_text(&section.heading)),
            );
        }
        for line in section.text.lines() {
            doc = doc.add_paragraph(Paragraph::new().add_run(Run::new().size(22).add_text(line)));
        }
        for bullet in &section.bullets {
            doc = doc.add_paragraph(
                Paragraph::new().add_run(Run::new().size(22).add_text(format!("• {bullet}"))),
            );
        }
    }
    let mut cursor = Cursor::new(Vec::new());
    doc.pack(&mut cursor).map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}

pub(crate) fn simple_wrap(text: &str, _width: usize) -> Vec<String> {
    let metrics = ttf_parser::Face::parse(include_bytes!("../assets/fonts/Lato-Regular.ttf"), 0)
        .expect("Bundled font is validated by PDF tests");
    let width = |s: &str| {
        s.chars()
            .filter_map(|c| metrics.glyph_index(c))
            .filter_map(|g| metrics.glyph_hor_advance(g))
            .map(|n| n as f32)
            .sum::<f32>()
            / metrics.units_per_em() as f32
            * 20.0
    };
    let mut result = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let proposed = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if width(&proposed) > 450.0 && !line.is_empty() {
            result.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        for c in word.chars() {
            if width(&format!("{line}{c}")) > 450.0 {
                result.push(std::mem::take(&mut line));
            }
            line.push(c);
        }
    }
    if !line.is_empty() {
        result.push(line);
    }
    result
}
pub(crate) fn pptx(title: &str, sections: &[Section]) -> Result<Vec<u8>, String> {
    use ppt_rs::generator::{SlideContent, create_pptx_with_content};
    if title.chars().count() > 90 || sections.iter().any(|s| s.heading.chars().count() > 90) {
        return Err(
            "Les titres PowerPoint sont limités à 90 caractères pour préserver la lisibilité"
                .into(),
        );
    }
    let mut cover = SlideContent::new(title);
    cover.title_size = Some(28);
    cover.title_color = Some("383385".into());
    let mut slides = vec![cover];
    for section in sections {
        let mut lines = Vec::new();
        for line in section.text.lines() {
            lines.extend(simple_wrap(line, 66));
        }
        for bullet in &section.bullets {
            lines.extend(simple_wrap(bullet, 66));
        }
        if lines.is_empty() {
            slides.push(SlideContent::new(&section.heading));
        }
        for (index, group) in lines.chunks(7).enumerate() {
            let heading = if index == 0 {
                section.heading.clone()
            } else {
                format!("{} (suite)", section.heading)
            };
            let mut slide = SlideContent::new(&heading);
            slide.title_size = Some(28);
            slide.content_size = Some(20);
            slide.title_color = Some("383385".into());
            for line in group {
                slide = slide.add_bullet(line);
            }
            slides.push(slide);
        }
    }
    if slides.len() > 150 {
        return Err("Présentation limitée à 150 diapositives".into());
    }
    create_pptx_with_content(title, slides).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sections() -> Vec<Section> {
        vec![Section {
            heading: "Résultats".into(),
            text: "Bonjour, été & création de documents.".into(),
            bullets: vec!["Premier élément".into(), "Deuxième élément".into()],
        }]
    }
    fn keep_qa(name: &str, bytes: &[u8]) {
        if let Some(dir) = std::env::var_os("MCP_WIN_QA_OUTPUT") {
            let dir = std::path::PathBuf::from(dir);
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(name), bytes).unwrap();
        }
    }
    fn validate_xml_package(bytes: &[u8]) {
        let mut package = office::archive(bytes).unwrap();
        assert!(package.by_name("[Content_Types].xml").is_ok());
        for i in 0..package.len() {
            let mut file = package.by_index(i).unwrap();
            if file.name().ends_with(".xml") || file.name().ends_with(".rels") {
                let mut xml = String::new();
                std::io::Read::read_to_string(&mut file, &mut xml).unwrap();
                roxmltree::Document::parse(&xml).unwrap();
            }
        }
    }
    #[test]
    fn office_generators_roundtrip_and_preserve_source_on_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: true,
        }];
        for ext in ["docx", "pptx"] {
            let name = format!("report.{ext}");
            document(&folders, "test", &name, "Rapport MCP Win", &sections()).unwrap();
            let bytes = fs::read(temp.path().join(&name)).unwrap();
            validate_xml_package(&bytes);
            keep_qa(&format!("report.{ext}"), &bytes);
            let (text, _) =
                office::read(&temp.path().join(&name), if ext == "pptx" { 2 } else { 1 }).unwrap();
            assert!(text.contains("Bonjour"), "{text}");
            let (modified, count) =
                office::replace_text(&bytes, ext, "Bonjour", "Bonsoir & merci", 1).unwrap();
            assert_eq!(count, 1);
            validate_xml_package(&modified);
            let copy = format!("copy.{ext}");
            writing::commit(&folders, "test", &copy, &modified, None).unwrap();
            let (text, _) =
                office::read(&temp.path().join(copy), if ext == "pptx" { 2 } else { 1 }).unwrap();
            assert!(text.contains("Bonsoir & merci"), "{text}");
            assert_eq!(fs::read(temp.path().join(&name)).unwrap(), bytes);
            assert!(document(&folders, "test", &name, "Overwrite", &sections()).is_err());
        }
    }
    #[test]
    fn pdf_is_paginated_and_accented_text_is_extractable() {
        let mut items = sections();
        items[0].text=format!("Bonjour, été & création de documents.\n{}", "Une ligne suffisamment longue pour vérifier le retour à la ligne, les marges et la pagination du document PDF généré en Rust.\n".repeat(90));
        let bytes = pdf("Rapport de validation", &items).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let pages = pdf_extract::extract_text_from_mem_by_pages(&bytes).unwrap();
        assert!(pages.len() >= 2);
        assert!(pages[0].contains("été"), "{}", pages[0]);
        keep_qa("report.pdf", &bytes);
    }
    #[test]
    fn spreadsheets_keep_types_formulas_and_save_an_edited_copy() {
        let temp = tempfile::tempdir().unwrap();
        let folders = vec![SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: true,
        }];
        let cells = vec![
            CellEdit {
                sheet: "Sheet1".into(),
                cell: "A1".into(),
                kind: CellKind::Text,
                value: "=literal".into(),
            },
            CellEdit {
                sheet: "Sheet1".into(),
                cell: "B2".into(),
                kind: CellKind::Number,
                value: "42.5".into(),
            },
            CellEdit {
                sheet: "Sheet1".into(),
                cell: "C2".into(),
                kind: CellKind::Formula,
                value: "SUM(B2,1)".into(),
            },
        ];
        let initial = spreadsheet(&folders, "test", "data.xlsx", None, None, &cells).unwrap();
        let source = fs::read(temp.path().join("data.xlsx")).unwrap();
        validate_xml_package(&source);
        keep_qa("data.xlsx", &source);
        let (text, _) = office::read(&temp.path().join("data.xlsx"), 1).unwrap();
        assert!(text.contains("=literal"));
        assert!(text.contains("42.5"));
        let edits = vec![CellEdit {
            sheet: "Sheet1".into(),
            cell: "A1".into(),
            kind: CellKind::Text,
            value: "Modifié".into(),
        }];
        spreadsheet(
            &folders,
            "test",
            "edited.xlsx",
            Some("data.xlsx"),
            initial["sha256"].as_str(),
            &edits,
        )
        .unwrap();
        assert_eq!(fs::read(temp.path().join("data.xlsx")).unwrap(), source);
        assert!(
            office::read(&temp.path().join("edited.xlsx"), 1)
                .unwrap()
                .0
                .contains("Modifié")
        );
        assert!(!valid_cell("A0"));
        assert!(!valid_cell("XFE1"));
        assert!(!valid_cell("A1:B2"));
    }
}

pub fn pdf(title: &str, sections: &[Section]) -> Result<Vec<u8>, String> {
    use printpdf::*;
    const FONT: &[u8] = include_bytes!("../assets/fonts/Lato-Regular.ttf");
    let metrics = ttf_parser::Face::parse(FONT, 0).map_err(|e| e.to_string())?;
    let measure = |text: &str, size: f32| -> f32 {
        text.chars()
            .filter_map(|c| metrics.glyph_index(c))
            .filter_map(|g| metrics.glyph_hor_advance(g))
            .map(|n| n as f32)
            .sum::<f32>()
            / metrics.units_per_em() as f32
            * size
    };
    let mut doc = PdfDocument::new(title);
    let mut warnings = Vec::new();
    let parsed = ParsedFont::from_bytes(FONT, 0, &mut warnings).ok_or("Police PDF invalide")?;
    let font = doc.add_font(&parsed);
    let mut paragraphs = vec![(title.to_owned(), 24.0_f32, true)];
    for section in sections {
        if !section.heading.is_empty() {
            paragraphs.push((section.heading.clone(), 15.0, true));
        }
        for line in section.text.lines() {
            paragraphs.push((line.to_owned(), 11.5, false));
        }
        for bullet in &section.bullets {
            paragraphs.push((format!("• {bullet}"), 11.5, false));
        }
    }
    let line_ops = |text: String, size: f32, y: f32, heading: bool| -> Vec<Op> {
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::External(font.clone()),
                size: Pt(size),
            },
            Op::SetTextCursor {
                pos: Point::new(Mm(19.0), Mm(y * 25.4 / 72.0)),
            },
            Op::SetFillColor {
                col: Color::Rgb(Rgb {
                    r: if heading { 0.22 } else { 0.14 },
                    g: if heading { 0.20 } else { 0.17 },
                    b: if heading { 0.52 } else { 0.22 },
                    icc_profile: None,
                }),
            },
            Op::ShowText {
                items: vec![TextItem::Text(text)],
            },
            Op::EndTextSection,
        ]
    };
    let mut pages: Vec<Vec<Op>> = vec![Vec::new()];
    let mut y = 782.0;
    for (text, size, heading) in paragraphs {
        for c in text.chars().filter(|c| !c.is_whitespace()) {
            if metrics.glyph_index(c).is_none() {
                return Err(format!(
                    "La police PDF ne contient pas le caractère {c:?}. Utilisez un texte couvert par Lato (latin, grec, cyrillique)."
                ));
            }
        }
        let mut lines = Vec::new();
        let mut current = String::new();
        for word in text.split_whitespace() {
            let proposed = if current.is_empty() {
                word.to_owned()
            } else {
                format!("{current} {word}")
            };
            if measure(&proposed, size) > 487.0 && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            if measure(word, size) > 487.0 {
                for c in word.chars() {
                    if measure(&format!("{current}{c}"), size) > 487.0 {
                        lines.push(std::mem::take(&mut current));
                    }
                    current.push(c);
                }
            } else {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(word);
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        let block_height = lines.len() as f32 * size * 1.4 + if heading { 28.0 } else { 6.0 };
        if block_height <= 720.0 && y - block_height < 58.0 && y < 782.0 {
            pages.push(Vec::new());
            y = 782.0;
        }
        for line in lines {
            if y - size * 1.4 < 58.0 {
                pages.push(Vec::new());
                y = 782.0;
            }
            if pages.len() > 100 {
                return Err("PDF limité à 100 pages".into());
            }
            pages
                .last_mut()
                .unwrap()
                .extend(line_ops(line, size, y, heading));
            y -= size * 1.4;
        }
        y -= if heading { 10.0 } else { 6.0 };
    }
    let total = pages.len();
    let pages = pages
        .into_iter()
        .enumerate()
        .map(|(index, mut ops)| {
            ops.extend(line_ops(
                format!("{} / {total}", index + 1),
                9.0,
                30.0,
                false,
            ));
            PdfPage::new(Mm(210.0), Mm(297.0), ops)
        })
        .collect();
    Ok(doc
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut Vec::new()))
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CellKind {
    Text,
    Number,
    Boolean,
    Formula,
}
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CellEdit {
    pub sheet: String,
    pub cell: String,
    pub kind: CellKind,
    pub value: String,
}
pub(crate) fn valid_cell(address: &str) -> bool {
    let Some(split) = address.find(|c: char| c.is_ascii_digit()) else {
        return false;
    };
    let (letters, digits) = address.split_at(split);
    if letters.is_empty()
        || letters.len() > 3
        || !letters.chars().all(|c| c.is_ascii_uppercase())
        || !digits.chars().all(|c| c.is_ascii_digit())
    {
        return false;
    }
    let column = letters
        .bytes()
        .fold(0_u32, |n, c| n * 26 + (c - b'A' + 1) as u32);
    column <= 16384
        && digits
            .parse::<u32>()
            .is_ok_and(|n| (1..=1048576).contains(&n))
}
pub fn spreadsheet(
    folders: &[SharedFolder],
    folder: &str,
    output: &str,
    input: Option<&str>,
    expected_hash: Option<&str>,
    cells: &[CellEdit],
) -> Result<Value, String> {
    writing::destination(folders, folder, output)?;
    if !output.to_lowercase().ends_with(".xlsx") {
        return Err("La sortie doit être un fichier .xlsx".into());
    }
    if cells.is_empty() || cells.len() > 10_000 {
        return Err("Fournir entre 1 et 10 000 cellules".into());
    }
    if cells
        .iter()
        .map(|cell| cell.sheet.as_str())
        .collect::<std::collections::HashSet<_>>()
        .len()
        > 500
    {
        return Err("Maximum 500 feuilles par appel".into());
    }
    let mut book = if let Some(input) = input {
        let source = library::resolve(folders, folder, input)?;
        if source
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("xlsx"))
        {
            return Err("Seuls les classeurs .xlsx peuvent être modifiés. Les formats .xls/.ods sont en lecture seule.".into());
        }
        if fs::metadata(&source).map_err(|e| e.to_string())?.len() > 25 * 1024 * 1024 {
            return Err("Source trop volumineuse".into());
        }
        let bytes = fs::read(source).map_err(|e| e.to_string())?;
        if Some(writing::hash(&bytes).as_str()) != expected_hash {
            return Err("Fournir le sha256 actuel du classeur source".into());
        }
        office::archive(&bytes)?;
        umya_spreadsheet::reader::xlsx::read_reader(Cursor::new(bytes), true)
            .map_err(|e| e.to_string())?
    } else {
        umya_spreadsheet::new_file()
    };
    for edit in cells {
        if !valid_cell(&edit.cell)
            || edit
                .value
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            || edit.value.len() > 32767
            || edit.sheet.is_empty()
            || edit.sheet.chars().count() > 31
            || edit.sheet.contains(['[', ']', ':', '*', '?', '/', '\\'])
        {
            return Err("Adresse de cellule, valeur ou nom de feuille invalide".into());
        }
        if book.sheet_by_name(&edit.sheet).is_err() {
            book.new_sheet(&edit.sheet).map_err(|e| e.to_string())?;
        }
        let cell = book
            .sheet_by_name_mut(&edit.sheet)
            .map_err(|e| e.to_string())?
            .cell_mut(edit.cell.as_str());
        match edit.kind {
            CellKind::Text => {
                cell.set_value_string(&edit.value);
            }
            CellKind::Number => {
                let number = edit.value.parse::<f64>().map_err(|_| "Nombre invalide")?;
                if !number.is_finite() {
                    return Err("Nombre non fini".into());
                }
                cell.set_value_number(number);
            }
            CellKind::Boolean => {
                cell.set_value_bool(
                    edit.value
                        .parse()
                        .map_err(|_| "Booléen attendu : true ou false")?,
                );
            }
            CellKind::Formula => {
                cell.set_formula(edit.value.trim_start_matches('='));
            }
        }
    }
    if input.is_none() {
        let brand = crate::v1::state::selected(None)?;
        for edit in cells {
            let sheet = book
                .sheet_by_name_mut(&edit.sheet)
                .map_err(|e| e.to_string())?;
            let row = edit
                .cell
                .trim_start_matches(|c: char| c.is_ascii_alphabetic())
                .parse::<u32>()
                .unwrap_or(1);
            let style = sheet.style_mut(edit.cell.as_str());
            style.alignment_mut().set_wrap_text(true);
            if row == 1 {
                style.set_background_color(format!("FF{}", brand.primary));
                style.font_mut().set_bold(true);
                style
                    .font_mut()
                    .color_mut()
                    .set_argb([255u8, 255, 255, 255]);
            } else if row % 2 == 0 {
                style.set_background_color("FFF7F8FC");
            }
            let column = edit.cell.trim_end_matches(|c: char| c.is_ascii_digit());
            let width = (edit.value.chars().count() as f64 + 3.0).clamp(14.0, 45.0);
            let current = sheet.column_dimension_mut(column).width();
            sheet
                .column_dimension_mut(column)
                .set_width(current.max(width));
        }
    }
    let mut bytes = Vec::new();
    umya_spreadsheet::writer::xlsx::write_writer(&book, &mut bytes).map_err(|e| e.to_string())?;
    let mut result = writing::commit(folders, folder, output, &bytes, None)?;
    result["warning"] = json!(
        "Les formules sont enregistrées mais pas évaluées. Les modifications de classeurs sont enregistrées dans un nouveau fichier ; vérifier les mises en forme complexes dans Excel ou LibreOffice."
    );
    Ok(result)
}

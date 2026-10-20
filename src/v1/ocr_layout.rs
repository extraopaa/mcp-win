use crate::{library::SharedFolder, ocr};
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize, schemars::JsonSchema)]
pub struct Args {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub page: Option<usize>,
    pub languages: Option<String>,
    pub output_path: Option<String>,
}
fn n(w: &Value, k: &str) -> f64 {
    w["box"][k].as_f64().unwrap_or(0.0)
}
pub fn structure(words: &[Value]) -> Value {
    let mut words = words.to_vec();
    words.sort_by(|a, b| {
        n(a, "y")
            .total_cmp(&n(b, "y"))
            .then(n(a, "x").total_cmp(&n(b, "x")))
    });
    let mut rows: Vec<Vec<Value>> = vec![];
    for w in words {
        let center = n(&w, "y") + n(&w, "height") / 2.0;
        let index = rows.iter().rposition(|r| {
            let a = &r[0];
            ((n(a, "y") + n(a, "height") / 2.0) - center).abs()
                < (n(a, "height").max(n(&w, "height")) * 0.55).max(4.0)
        });
        if let Some(i) = index {
            rows[i].push(w)
        } else {
            rows.push(vec![w])
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| n(a, "x").total_cmp(&n(b, "x")));
    }
    let median_height = {
        let mut heights: Vec<f64> = rows.iter().flatten().map(|w| n(w, "height")).collect();
        heights.sort_by(f64::total_cmp);
        heights.get(heights.len() / 2).copied().unwrap_or(16.0)
    };
    let mut blocks = vec![];
    let mut candidates: Vec<Vec<(f64, String, f64)>> = vec![];
    let mut fields = vec![];
    let field_re = regex::Regex::new(
        r"(?i)^(total\s*(?:ttc|ht)?|tva|date|facture|siret|iban|référence)\s*[:\-]?\s+(.+)$",
    )
    .unwrap();
    for (i, row) in rows.iter().enumerate() {
        let text = row
            .iter()
            .filter_map(|w| w["text"].as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let confidence = row
            .iter()
            .filter_map(|w| w["confidence"].as_f64())
            .sum::<f64>()
            / row.len().max(1) as f64;
        let x = row.iter().map(|w| n(w, "x")).fold(f64::INFINITY, f64::min);
        let y = row.iter().map(|w| n(w, "y")).fold(f64::INFINITY, f64::min);
        let right = row
            .iter()
            .map(|w| n(w, "x") + n(w, "width"))
            .fold(0.0, f64::max);
        let bottom = row
            .iter()
            .map(|w| n(w, "y") + n(w, "height"))
            .fold(0.0, f64::max);
        let heading = row.len() < 15
            && row.iter().map(|w| n(w, "height")).sum::<f64>() / row.len().max(1) as f64
                > median_height * 1.25;
        blocks.push(json!({"id":format!("ocr-line-{}",i+1),"kind":if heading{"heading"}else{"paragraph"},"text":text,"confidence":confidence,"needs_review":confidence<80.0,"box":{"x":x,"y":y,"width":right-x,"height":bottom-y}}));
        if let Some(c) = field_re.captures(&text) {
            fields.push(json!({"name":&c[1],"value":&c[2],"confidence":confidence,"line":i+1,"needs_review":true}))
        }
        let mut cells: Vec<(f64, String, f64)> = vec![];
        let mut previous_right = 0.0;
        for w in row {
            let x = n(w, "x");
            let s = w["text"].as_str().unwrap_or("");
            let c = w["confidence"].as_f64().unwrap_or(0.0);
            if cells.is_empty() || x - previous_right > median_height * 1.8 {
                cells.push((x, s.into(), c))
            } else {
                let last = cells.last_mut().unwrap();
                last.1.push(' ');
                last.1.push_str(s);
                last.2 = last.2.min(c)
            }
            previous_right = x + n(w, "width");
        }
        if cells.len() >= 2 {
            candidates.push(cells)
        }
    }
    let mut tables = vec![];
    let mut used = std::collections::HashSet::new();
    for i in 0..candidates.len() {
        if used.contains(&i) {
            continue;
        }
        let count = candidates[i].len();
        let mut group = vec![i];
        for j in i + 1..candidates.len() {
            if !used.contains(&j)
                && candidates[j].len() == count
                && candidates[j]
                    .iter()
                    .zip(&candidates[i])
                    .all(|(a, b)| (a.0 - b.0).abs() < median_height * 2.5)
            {
                group.push(j)
            }
        }
        if group.len() < 3 {
            continue;
        }
        for j in &group {
            used.insert(*j);
        }
        let rows: Vec<Vec<String>> = group
            .iter()
            .map(|j| candidates[*j].iter().map(|c| c.1.clone()).collect())
            .collect();
        let confidence = group
            .iter()
            .flat_map(|j| candidates[*j].iter())
            .map(|c| c.2)
            .fold(100.0, f64::min);
        tables.push(json!({"id":format!("ocr-table-{}",tables.len()+1),"columns":rows[0],"rows":rows[1..],"confidence":confidence,"needs_review":true,"method":"alignement géométrique heuristique"}));
    }
    json!({"blocks":blocks,"tables":tables,"fields":fields,"reading_order":"haut vers bas, puis gauche à droite ; colonnes complexes à vérifier","warning":"Reconstruction heuristique : les tableaux et valeurs extraites doivent être validés, même si la confiance OCR est élevée."})
}
pub async fn extract(folders: Vec<SharedFolder>, args: Args) -> Result<Value, String> {
    let full = crate::advanced::input(&folders, &args.folder, &args.path)?;
    let hash = crate::writing::hash(&std::fs::read(full).map_err(|e| e.to_string())?);
    let ocr = ocr::recognize(
        folders.clone(),
        ocr::OcrArgs {
            folder: args.folder.clone(),
            path: args.path.clone(),
            page: args.page,
            languages: args.languages,
            layout: Some("auto".into()),
            rotate: None,
            plain_text_only: Some(false),
            offset: None,
            limit: None,
        },
    )
    .await?;
    let mut result = structure(ocr["words"].as_array().map(Vec::as_slice).unwrap_or(&[]));
    result["page"] = ocr["page"].clone();
    result["pages"] = ocr["pages"].clone();
    result["source"] = json!({"path":args.path,"sha256":hash,"page":args.page.unwrap_or(1)});
    result["text"] = ocr["text"].clone();
    if let Some(output) = args.output_path {
        let table = result["tables"]
            .as_array()
            .and_then(|v| v.first())
            .ok_or("Aucun tableau stable détecté : consulter les blocs et corriger manuellement")?;
        let rows = std::iter::once(table["columns"].as_array().unwrap()).chain(
            table["rows"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_array),
        );
        if output.to_ascii_lowercase().ends_with(".csv") {
            let mut writer = csv::WriterBuilder::new()
                .delimiter(b';')
                .from_writer(Vec::new());
            for row in rows {
                writer
                    .write_record(row.iter().map(|v| v.as_str().unwrap_or("")))
                    .map_err(|e| e.to_string())?;
            }
            let bytes = writer.into_inner().map_err(|e| e.to_string())?;
            result["export"] =
                crate::writing::commit(&folders, &args.folder, &output, &bytes, None)?;
        } else if output.to_ascii_lowercase().ends_with(".xlsx") {
            let mut cells = vec![];
            for (r, row) in rows.enumerate() {
                if r > 1000 || row.len() > 26 {
                    return Err("Export limité à 1000 lignes et 26 colonnes".into());
                }
                for (c, value) in row.iter().enumerate() {
                    cells.push(crate::generate::CellEdit {
                        sheet: "OCR à vérifier".into(),
                        cell: format!("{}{}", (b'A' + c as u8) as char, r + 1),
                        kind: crate::generate::CellKind::Text,
                        value: value.as_str().unwrap_or("").into(),
                    })
                }
            }
            result["export"] =
                crate::generate::spreadsheet(&folders, &args.folder, &output, None, None, &cells)?;
        } else {
            return Err("Export OCR : CSV ou XLSX".into());
        }
    }
    Ok(result)
}

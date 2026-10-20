use super::{
    design::{self, Block, escape},
    state::Brand,
};
use std::collections::BTreeMap;
struct Slide {
    title: String,
    lines: Vec<String>,
    table: Option<ppt_rs::generator::table::Table>,
    image: Option<Vec<u8>>,
}
fn text(
    id: usize,
    name: &str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    size: u32,
    color: &str,
    lines: &[String],
    bold: bool,
    font: &str,
) -> String {
    let paragraphs=lines.iter().map(|line|format!("<a:p><a:pPr marL=\"0\" indent=\"0\"><a:spcAft><a:spcPts val=\"800\"/></a:spcAft></a:pPr><a:r><a:rPr lang=\"fr-FR\" sz=\"{}\" b=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:latin typeface=\"{}\"/></a:rPr><a:t>{}</a:t></a:r></a:p>",size*100,if bold{1}else{0},color,escape(font),escape(line))).collect::<String>();
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap=\"square\" lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\"/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>",
        escape(name)
    )
}
fn rectangle(color: &str, w: u32, h: u32) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Accent\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val=\"{color}\"/></a:solidFill><a:ln><a:noFill/></a:ln></p:spPr></p:sp>"
    )
}
fn picture(
    bytes: &[u8],
    id: usize,
    rel: &str,
    area: (u32, u32, u32, u32),
) -> Result<(Vec<u8>, String), String> {
    let im = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let data = crate::advanced::png(&im)?;
    let scale =
        (area.2 as f64 / im.width().max(1) as f64).min(area.3 as f64 / im.height().max(1) as f64);
    let w = (im.width() as f64 * scale) as u32;
    let h = (im.height() as f64 * scale) as u32;
    let x = area.0 + (area.2 - w) / 2;
    let y = area.1 + (area.3 - h) / 2;
    Ok((
        data,
        format!(
            "<p:pic><p:nvPicPr><p:cNvPr id=\"{id}\" name=\"Illustration\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"{rel}\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>"
        ),
    ))
}
pub fn create(
    title: &str,
    blocks: &[Block],
    brand: &Brand,
    images: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    use ppt_rs::generator::{
        SlideContent, create_pptx_with_content,
        table::{CellAlign, Table, TableCell, TableRow, generate_table_xml},
    };
    if title.chars().count() > 140 {
        return Err("Titre PowerPoint : 140 caractères maximum".into());
    }
    let mut slides = vec![Slide {
        title: title.into(),
        lines: vec![brand.organization.clone(), brand.contact.clone()],
        table: None,
        image: None,
    }];
    let mut heading = title.to_owned();
    let mut lines = vec![];
    fn flush(slides: &mut Vec<Slide>, heading: &str, lines: &mut Vec<String>) {
        if !lines.is_empty() {
            for chunk in lines.chunks(8) {
                slides.push(Slide {
                    title: heading.into(),
                    lines: chunk.to_vec(),
                    table: None,
                    image: None,
                })
            }
            lines.clear()
        }
    }
    for b in blocks {
        match b.kind.as_str() {
            "heading" => {
                flush(&mut slides, &heading, &mut lines);
                heading = b.text.clone();
            }
            "table" => {
                flush(&mut slides, &heading, &mut lines);
                if b.columns.len() > 8 {
                    return Err("Tableau PowerPoint : maximum 8 colonnes lisibles ; utiliser Excel ou PDF pour un tableau plus large".into());
                }
                let font = if b.columns.len() > 4 { 12 } else { 16 };
                let column = 7_800_000 / b.columns.len() as u32;
                let chars = (column as f64 / 12700.0 / (font as f64 * 0.55)).max(1.0);
                let make_row = |values: &[String], header: bool| -> Result<TableRow, String> {
                    let height = (values
                        .iter()
                        .map(|s| (s.chars().count() as f64 / chars).ceil())
                        .fold(1.0, f64::max)
                        * font as f64
                        * 1.3
                        + 15.0)
                        * 12700.0;
                    if height > 3_500_000.0 {
                        return Err(
                            "Cellule trop longue pour une diapositive : résumer ou utiliser un PDF"
                                .into(),
                        );
                    }
                    let cells = values
                        .iter()
                        .map(|s| {
                            let mut c = TableCell::new(s);
                            c.bold = header;
                            c.text_color = Some(if header {
                                "FFFFFF".into()
                            } else {
                                "1F2937".into()
                            });
                            c.background_color = Some(if header {
                                brand.primary.clone()
                            } else {
                                "F7F8FC".into()
                            });
                            c.font_size = Some(font);
                            c.font_family = Some(brand.font.clone());
                            c.align = CellAlign::Left;
                            c
                        })
                        .collect();
                    Ok(TableRow::new(cells).with_height(height as u32))
                };
                let header = make_row(&b.columns, true)?;
                let mut rows = vec![header.clone()];
                let mut height = header.height.unwrap_or(450000);
                for values in &b.rows {
                    let row = make_row(values, false)?;
                    let h = row.height.unwrap_or(450000);
                    if height + h > 4_500_000 && rows.len() > 1 {
                        slides.push(Slide {
                            title: heading.clone(),
                            lines: vec![],
                            table: Some(Table::new(
                                rows,
                                vec![column; b.columns.len()],
                                650000,
                                1500000,
                            )),
                            image: None,
                        });
                        rows = vec![header.clone()];
                        height = header.height.unwrap_or(450000)
                    }
                    height += h;
                    rows.push(row);
                }
                slides.push(Slide {
                    title: heading.clone(),
                    lines: vec![],
                    table: Some(Table::new(
                        rows,
                        vec![column; b.columns.len()],
                        650000,
                        1500000,
                    )),
                    image: None,
                });
            }
            "image" => {
                flush(&mut slides, &heading, &mut lines);
                slides.push(Slide {
                    title: if b.text.is_empty() {
                        heading.clone()
                    } else {
                        b.text.clone()
                    },
                    lines: vec![],
                    table: None,
                    image: images.get(&b.id).cloned(),
                });
            }
            "list" => {
                for line in &b.items {
                    lines.extend(crate::generate::simple_wrap(&format!("• {line}"), 60))
                }
            }
            _ => {
                lines.extend(crate::generate::simple_wrap(&b.text, 60));
            }
        }
    }
    flush(&mut slides, &heading, &mut lines);
    if slides.len() > 150 {
        return Err("Présentation limitée à 150 diapositives".into());
    }
    let mut parts = design::unpack(
        &create_pptx_with_content(
            title,
            slides.iter().map(|s| SlideContent::new(&s.title)).collect(),
        )
        .map_err(|e| e.to_string())?,
    )?;
    let logo = super::state::logo(brand)?;
    for (i, slide) in slides.iter().enumerate() {
        let cover = i == 0;
        let mut shapes = rectangle(
            &brand.primary,
            if cover { 160000 } else { 9_144_000 },
            if cover { 6_858_000 } else { 70000 },
        );
        let color = if cover {
            "FFFFFF"
        } else {
            brand.primary.as_str()
        };
        shapes.push_str(&text(
            3,
            "Titre",
            700000,
            if cover { 1300000 } else { 430000 },
            7_800_000,
            if cover { 3_000_000 } else { 1_000_000 },
            if cover { 36 } else { 27 },
            color,
            &[slide.title.clone()],
            true,
            &brand.font,
        ));
        if !slide.lines.is_empty() {
            shapes.push_str(&text(
                4,
                "Contenu",
                700000,
                if cover { 4_900_000 } else { 1_550_000 },
                7_750_000,
                if cover { 1_400_000 } else { 4_550_000 },
                if cover { 16 } else { 20 },
                if cover { "C3CBE0" } else { "374151" },
                &slide.lines,
                false,
                &brand.font,
            ))
        }
        if let Some(table) = &slide.table {
            shapes.push_str(&generate_table_xml(table, 5))
        }
        let image = if cover {
            logo.as_ref()
        } else {
            slide.image.as_ref()
        };
        if let Some(image) = image {
            let (data, xml) = picture(
                image,
                6,
                "illustration",
                if cover {
                    (7_300_000, 350_000, 1_100_000, 700_000)
                } else {
                    (720_000, 1_450_000, 7_700_000, 4_400_000)
                },
            )?;
            parts.insert(format!("ppt/media/studio-{}.png", i + 1), data);
            let rel = format!("ppt/slides/_rels/slide{}.xml.rels", i + 1);
            let existing=String::from_utf8(parts.get(&rel).cloned().unwrap_or_else(||b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"></Relationships>".to_vec())).map_err(|e|e.to_string())?;
            parts.insert(rel,existing.replace("</Relationships>",&format!("<Relationship Id=\"illustration\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/studio-{}.png\"/></Relationships>",i+1)).into_bytes());
            shapes.push_str(&xml);
        }
        shapes.push_str(&text(
            9,
            "Pagination",
            700000,
            6_350_000,
            7_750_000,
            250000,
            9,
            if cover { "AAB5D0" } else { "8E94A7" },
            &[format!(
                "{}    ·    {} / {}",
                brand.footer,
                i + 1,
                slides.len()
            )],
            false,
            &brand.font,
        ));
        let xml = format!(
            "<p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
            if cover { "13162B" } else { "FFFFFF" }
        );
        parts.insert(format!("ppt/slides/slide{}.xml", i + 1), xml.into_bytes());
    }
    if let Some(content) = parts.get_mut("[Content_Types].xml") {
        let xml = String::from_utf8(content.clone()).map_err(|e| e.to_string())?;
        if !xml.contains("image/png") {
            *content = xml
                .replace(
                    "</Types>",
                    "<Default Extension=\"png\" ContentType=\"image/png\"/></Types>",
                )
                .into_bytes()
        }
    }
    design::pack(parts)
}

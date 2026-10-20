use super::{quality, state};
use crate::{
    advanced, generate,
    library::{self, SharedFolder},
    writing,
};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
    path::Path,
};
#[derive(Clone, Default, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct Block {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub level: u8,
    pub items: Vec<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub path: String,
}
#[derive(Clone, Default, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct Citation {
    pub folder: String,
    pub path: String,
    pub sha256: String,
    pub page: Option<usize>,
    pub sheet: Option<String>,
    pub line: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct Args {
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) internal_brand: Option<state::Brand>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) internal_asset_hashes: BTreeMap<String, String>,
    pub template: Option<String>,
    pub data: Option<Value>,
    pub folder: String,
    pub path: String,
    pub title: String,
    pub sections: Vec<generate::Section>,
    pub blocks: Vec<Block>,
    /// professional (défaut), report, quote, invoice, commercial, course, presentation, minimal.
    pub style: String,
    pub brand: Option<String>,
    pub cover: Option<bool>,
    pub table_of_contents: Option<bool>,
    /// Le mode brut doit être explicitement demandé ; false applique une vraie mise en page.
    pub raw: bool,
    pub citations: Vec<Citation>,
}
impl Default for Args {
    fn default() -> Self {
        Self {
            internal_brand: None,
            internal_asset_hashes: BTreeMap::new(),
            template: None,
            data: None,
            folder: String::new(),
            path: String::new(),
            title: String::new(),
            sections: vec![],
            blocks: vec![],
            style: "professional".into(),
            brand: None,
            cover: None,
            table_of_contents: None,
            raw: false,
            citations: vec![],
        }
    }
}
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub fn blocks(args: &Args) -> Result<Vec<Block>, String> {
    if ![
        "professional",
        "report",
        "quote",
        "invoice",
        "commercial",
        "course",
        "presentation",
        "minimal",
    ]
    .contains(&args.style.as_str())
    {
        return Err("Style inconnu : professional, report, quote, invoice, commercial, course, presentation ou minimal".into());
    }
    let mut blocks = args.blocks.clone();
    for s in &args.sections {
        if !s.heading.is_empty() {
            blocks.push(Block {
                kind: "heading".into(),
                text: s.heading.clone(),
                level: 1,
                ..Default::default()
            })
        }
        for paragraph in s.text.split("\n\n") {
            if !paragraph.is_empty() {
                blocks.push(Block {
                    kind: "paragraph".into(),
                    text: paragraph.into(),
                    ..Default::default()
                })
            }
        }
        if !s.bullets.is_empty() {
            blocks.push(Block {
                kind: "list".into(),
                items: s.bullets.clone(),
                ..Default::default()
            })
        }
    }
    if blocks.len() > 500
        || args.title.len() > 500
        || serde_json::to_vec(&blocks)
            .map_err(|e| e.to_string())?
            .len()
            > 300_000
    {
        return Err("Document limité à 500 blocs, 300 Ko de contenu, titre de 500 octets".into());
    }
    let mut ids = std::collections::HashSet::new();
    for (i, b) in blocks.iter_mut().enumerate() {
        if b.id.is_empty() {
            b.id = format!("block-{}", i + 1)
        }
        if b.id.len() > 100 || !ids.insert(b.id.clone()) {
            return Err("Identifiants de blocs uniques requis".into());
        }
        if b.kind.is_empty() {
            b.kind = "paragraph".into()
        }
        if !["heading", "paragraph", "table", "callout", "image", "list"].contains(&b.kind.as_str())
        {
            return Err("Bloc : heading, paragraph, table, callout, image ou list".into());
        }
        b.level = b.level.clamp(1, 3);
        if b.kind == "table"
            && (b.columns.is_empty()
                || b.columns.len() > 20
                || b.rows.len() > 1000
                || b.rows.iter().any(|r| r.len() != b.columns.len()))
        {
            return Err("Table : 1 à 20 colonnes, 1000 lignes maximum et cellules alignées sur les colonnes".into());
        }
    }
    Ok(blocks)
}
const TYPST: &str = r##"#let d = json("mcp-data.json")
#let accent = rgb("#" + d.brand.primary)
#let ink = rgb("#1F2937")
#set document(title:d.title,author:d.brand.organization)
#set text(font:d.brand.font,size:d.brand.body_size_pt*1pt,fill:ink,lang:"fr")
#set par(justify:true,leading:0.72em,spacing:0.9em)
#set page(paper:d.brand.page_size,margin:d.brand.margin_mm*1mm,header:context [#text(size:8pt,fill:accent,d.brand.organization) #h(1fr) #text(size:8pt,fill:gray,d.style)],footer:context [#line(length:100%,stroke:0.4pt+rgb("#e5e7eb")) #v(3pt) #text(size:8pt,fill:gray,d.brand.footer) #h(1fr) #text(size:8pt,counter(page).display("1 / 1",both:true))])
#show heading: it => block(above:1.4em,below:0.7em,sticky:true)[#text(fill:accent,weight:"bold",it.body)]
#set heading(numbering:none)
#if d.has_logo { image("brand-logo.png",width:d.brand.logo_width_mm*1mm,height:25mm,fit:"contain"); v(8mm) }
#if d.cover {v(25mm)}
#text(size:if d.cover {32pt}else{26pt},weight:"bold",fill:accent,d.title)
#v(5mm)
#if d.brand.contact != "" {text(size:9pt,fill:gray,d.brand.contact);v(4mm)}
#line(length:if d.cover {35%}else{100%},stroke:2pt+accent)
#v(8mm)
#if d.cover {v(1fr);text(size:11pt,d.brand.organization);pagebreak()}
#if d.toc {outline(title:[Sommaire],depth:2);pagebreak()}
#for b in d.blocks {
 if b.kind == "heading" {heading(level:b.level,text(b.text))}
 else if b.kind == "paragraph" {text(b.text);parbreak()}
 else if b.kind == "callout" {block(width:100%,fill:rgb("#F5F6FA"),inset:12pt,radius:5pt,breakable:true)[#text(fill:accent,weight:"bold",b.text)];v(8pt)}
 else if b.kind == "list" {list(..b.items.map(s=>text(s)));parbreak()}
 else if b.kind == "image" {block(width:100%)[#align(center,image(b.asset_name,width:90%,height:75mm,fit:"contain")) #if b.text != "" {align(center,text(size:9pt,fill:gray,b.text))}];v(8pt)}
 else if b.kind == "table" {table(columns:(1fr,)*b.columns.len(),inset:8pt,stroke:0.4pt+rgb("#E5E7EB"),fill:(x,y)=>if y==0{accent}else if calc.odd(y){rgb("#F7F8FC")}else{white},table.header(..b.columns.map(s=>text(weight:"bold",fill:white,s))),..b.rows.flatten().map(s=>text(size:9pt,s)));v(10pt)}
}
#if d.sources.len()>0 {heading(level:1,[Sources]);for s in d.sources {text(size:8pt,s);parbreak()}}
"##;
fn style_label(style: &str) -> &str {
    match style {
        "report" => "RAPPORT",
        "quote" => "DEVIS",
        "invoice" => "FACTURE",
        "commercial" => "DOSSIER COMMERCIAL",
        "course" => "COURS",
        "presentation" => "PRÉSENTATION",
        _ => "DOCUMENT",
    }
}
pub fn pack(parts: BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        if name.ends_with(".xml") || name.ends_with(".rels") {
            roxmltree::Document::parse(std::str::from_utf8(&bytes).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .map_err(|e| e.to_string())?;
        zip.write_all(&bytes).map_err(|e| e.to_string())?;
    }
    Ok(zip.finish().map_err(|e| e.to_string())?.into_inner())
}
pub fn unpack(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    use std::io::Read;
    let mut zip = crate::office::archive(bytes)?;
    let mut result = BTreeMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let mut bytes = vec![];
        f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        result.insert(f.name().into(), bytes);
    }
    Ok(result)
}
fn p(text: &str, style: &str) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"{}\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        style,
        escape(text)
    )
}
fn docx(
    title: &str,
    blocks: &[Block],
    brand: &state::Brand,
    cover: bool,
    toc: bool,
    sources: &[String],
    images: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let mut parts = BTreeMap::new();
    let mut body = String::new();
    let mut rels = String::from(
        "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"styles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/><Relationship Id=\"header\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header1.xml\"/><Relationship Id=\"footer\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer\" Target=\"footer1.xml\"/><Relationship Id=\"settings\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings\" Target=\"settings.xml\"/>",
    );
    let mut add_image = |name: &str, bytes: &[u8], large: bool| -> Result<String, String> {
        let format = image::guess_format(bytes).map_err(|e| e.to_string())?;
        let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(64 * 1024 * 1024);
        limits.max_image_width = Some(8000);
        limits.max_image_height = Some(8000);
        reader.limits(limits);
        let im = reader.decode().map_err(|e| e.to_string())?;
        let data = advanced::png(&im)?;
        let id = format!("img{}", parts.len() + 1);
        let max_width = if large {
            5_400_000f64
        } else {
            brand.logo_width_mm as f64 * 36000.0
        };
        let max_height = if large { 3_000_000f64 } else { 850_000f64 };
        let scale =
            (max_width / im.width().max(1) as f64).min(max_height / im.height().max(1) as f64);
        let width = (im.width() as f64 * scale) as u64;
        let height = (im.height() as f64 * scale) as u64;
        parts.insert(format!("word/media/{id}.png"), data);
        rels.push_str(&format!("<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/{id}.png\"/>"));
        Ok(format!(
            "<w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"{width}\" cy=\"{height}\"/><wp:docPr id=\"{}\" name=\"{}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"0\" name=\"{}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{width}\" cy=\"{height}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>",
            parts.len(),
            escape(name),
            escape(name)
        ))
    };
    if let Some(logo) = state::logo(brand)? {
        body.push_str(&add_image("Logo", &logo, false)?)
    }
    body.push_str(&p(title, "Title"));
    if !brand.contact.is_empty() {
        body.push_str(&p(&brand.contact, "Subtitle"))
    }
    if cover {
        body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>")
    }
    if toc {
        body.push_str(&p("Sommaire", "Heading1"));
        body.push_str("<w:p><w:fldSimple w:instr=\"TOC \\o &quot;1-3&quot; \\h\"><w:r><w:t>Mettre à jour le sommaire dans le traitement de texte.</w:t></w:r></w:fldSimple></w:p>")
    }
    for b in blocks {
        match b.kind.as_str() {
            "heading" => body.push_str(&p(&b.text, &format!("Heading{}", b.level))),
            "list" => {
                for item in &b.items {
                    body.push_str(&p(&format!("• {item}"), "ListParagraph"))
                }
            }
            "table" => {
                body.push_str("<w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblBorders><w:bottom w:val=\"single\" w:sz=\"4\" w:color=\"E5E7EB\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:color=\"E5E7EB\"/></w:tblBorders></w:tblPr><w:tblGrid>");
                for _ in &b.columns {
                    body.push_str("<w:gridCol w:w=\"2000\"/>")
                }
                body.push_str("</w:tblGrid>");
                for (i, row) in std::iter::once(&b.columns).chain(b.rows.iter()).enumerate() {
                    body.push_str("<w:tr>");
                    if i == 0 {
                        body.push_str("<w:trPr><w:tblHeader/></w:trPr>")
                    }
                    for cell in row {
                        let color = if i == 0 {
                            brand.primary.as_str()
                        } else if i % 2 == 1 {
                            "F7F8FC"
                        } else {
                            "FFFFFF"
                        };
                        body.push_str(&format!("<w:tc><w:tcPr><w:shd w:fill=\"{color}\"/><w:tcMar><w:top w:w=\"110\" w:type=\"dxa\"/><w:bottom w:w=\"110\" w:type=\"dxa\"/><w:left w:w=\"110\" w:type=\"dxa\"/></w:tcMar></w:tcPr>{}</w:tc>",p(cell,if i==0{"TableHeader"}else{"Normal"})))
                    }
                    body.push_str("</w:tr>")
                }
                body.push_str("</w:tbl>");
            }
            "callout" => body.push_str(&p(&b.text, "Callout")),
            "image" => {
                if let Some(bytes) = images.get(&b.id) {
                    body.push_str(&add_image(&b.text, bytes, true)?);
                    if !b.text.is_empty() {
                        body.push_str(&p(&b.text, "Caption"))
                    }
                }
            }
            _ => body.push_str(&p(&b.text, "Normal")),
        }
    }
    if !sources.is_empty() {
        body.push_str(&p("Sources", "Heading1"));
        for source in sources {
            body.push_str(&p(source, "Caption"))
        }
    }
    let (page_w, page_h) = if brand.page_size == "us-letter" {
        (12240, 15840)
    } else {
        (11906, 16838)
    };
    let margin = (brand.margin_mm as f64 * 1440.0 / 25.4).round() as u32;
    body.push_str(&format!("<w:sectPr><w:headerReference w:type=\"default\" r:id=\"header\"/><w:footerReference w:type=\"default\" r:id=\"footer\"/><w:pgSz w:w=\"{page_w}\" w:h=\"{page_h}\"/><w:pgMar w:top=\"{margin}\" w:right=\"{margin}\" w:bottom=\"{margin}\" w:left=\"{margin}\" w:header=\"600\" w:footer=\"600\"/></w:sectPr>"));
    rels.push_str("</Relationships>");
    let styles=format!(r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}"/><w:sz w:val="{body_size}"/><w:color w:val="1F2937"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="160" w:line="288" w:lineRule="auto"/><w:widowControl/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style><w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:pPr><w:spacing w:before="240" w:after="400"/><w:keepNext/></w:pPr><w:rPr><w:b/><w:color w:val="{accent}"/><w:sz w:val="56"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Subtitle"><w:rPr><w:color w:val="8E94A7"/></w:rPr></w:style>{headings}<w:style w:type="paragraph" w:styleId="ListParagraph"><w:pPr><w:ind w:left="300" w:hanging="180"/></w:pPr></w:style><w:style w:type="paragraph" w:styleId="Callout"><w:pPr><w:shd w:fill="F0EDFF"/><w:spacing w:before="160" w:after="200"/></w:pPr><w:rPr><w:b/><w:color w:val="{accent}"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="TableHeader"><w:rPr><w:b/><w:color w:val="FFFFFF"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Caption"><w:rPr><w:sz w:val="18"/><w:color w:val="6B7280"/></w:rPr></w:style></w:styles>"#,font=escape(&brand.font),body_size=brand.body_size_pt as u16*2,accent=brand.primary,headings=(1..=3).map(|n|format!("<w:style w:type=\"paragraph\" w:styleId=\"Heading{n}\"><w:name w:val=\"heading {n}\"/><w:pPr><w:keepNext/><w:keepLines/><w:outlineLvl w:val=\"{}\"/><w:spacing w:before=\"300\" w:after=\"160\"/></w:pPr><w:rPr><w:b/><w:color w:val=\"{}\"/><w:sz w:val=\"{}\"/></w:rPr></w:style>",n-1,brand.primary,36-n*4)).collect::<String>());
    parts.insert("word/styles.xml".into(), styles.into_bytes());
    parts.insert("word/_rels/document.xml.rels".into(), rels.into_bytes());
    parts.insert("word/document.xml".into(),format!("<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>{body}</w:body></w:document>").into_bytes());
    parts.insert("word/header1.xml".into(),format!("<w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">{}</w:hdr>",p(&brand.organization,"Caption")).into_bytes());
    parts.insert("word/footer1.xml".into(),format!("<w:ftr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:p><w:pPr><w:jc w:val=\"right\"/></w:pPr><w:r><w:rPr><w:sz w:val=\"16\"/></w:rPr><w:t>{} · </w:t></w:r><w:fldSimple w:instr=\"PAGE\"/><w:r><w:t> / </w:t></w:r><w:fldSimple w:instr=\"NUMPAGES\"/></w:p></w:ftr>",escape(&brand.footer)).into_bytes());
    parts.insert("word/settings.xml".into(),b"<w:settings xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:updateFields w:val=\"true\"/></w:settings>".to_vec());
    parts.insert("_rels/.rels".into(),b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"doc\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>".to_vec());
    parts.insert("[Content_Types].xml".into(),b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/><Override PartName=\"/word/header1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml\"/><Override PartName=\"/word/footer1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml\"/><Override PartName=\"/word/settings.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml\"/></Types>".to_vec());
    pack(parts)
}
pub async fn create(folders: &[SharedFolder], mut args: Args) -> Result<Value, String> {
    if let Some(template) = args.template.clone() {
        let data = args.data.unwrap_or_else(
            || json!({"title":args.title,"sections":args.sections,"blocks":args.blocks}),
        );
        return crate::templates::create(
            folders.to_vec(),
            crate::templates::TemplateArgs {
                folder: args.folder,
                template,
                output_path: args.path,
                data,
                assets: vec![],
                collision_policy: None,
            },
        )
        .await;
    }
    writing::destination(folders, &args.folder, &args.path)?;
    let mut content = blocks(&args)?;
    if args.title.trim().is_empty() {
        args.title = Path::new(&args.path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into()
    }
    let brand = if let Some(brand) = args.internal_brand.clone() {
        brand
    } else {
        state::selected(args.brand.as_deref())?
    };
    let mut sources = vec![];
    for c in &args.citations {
        if sources.len() >= 100 {
            return Err("100 sources maximum".into());
        }
        let folder = if c.folder.is_empty() {
            &args.folder
        } else {
            &c.folder
        };
        let path = advanced::input(folders, folder, &c.path)?;
        let hash = writing::hash(&fs::read(path).map_err(|e| e.to_string())?);
        if c.sha256.is_empty() || hash != c.sha256 {
            return Err(format!("Source modifiée ou empreinte absente : {}", c.path));
        }
        sources.push(format!(
            "{}{}{}{} · SHA-256 {}",
            c.path,
            c.page.map(|p| format!(" · p. {p}")).unwrap_or_default(),
            c.sheet
                .as_ref()
                .map(|s| format!(" · {s}"))
                .unwrap_or_default(),
            c.line.map(|l| format!(" · ligne {l}")).unwrap_or_default(),
            &hash[..12]
        ));
    }
    let headings = content.iter().filter(|b| b.kind == "heading").count();
    let cover = args
        .cover
        .unwrap_or(content.iter().map(|b| b.text.len()).sum::<usize>() > 9000 || headings >= 6);
    let toc = args.table_of_contents.unwrap_or(headings >= 6);
    let ext = Path::new(&args.path)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let mut assets = vec![];
    let mut images = BTreeMap::new();
    let mut model_blocks = content.clone();
    for (i, b) in content.iter_mut().enumerate() {
        if b.kind == "image" {
            if images.len() >= 10 {
                return Err("10 images maximum".into());
            }
            let path = advanced::input(folders, &args.folder, &b.path)?;
            let bytes = fs::read(path).map_err(|e| e.to_string())?;
            if bytes.len() > 5 * 1024 * 1024 {
                return Err("Image limitée à 5 Mio".into());
            }
            let format = image::guess_format(&bytes).map_err(|_| "Image PNG/JPEG requise")?;
            if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
                return Err("Image PNG/JPEG requise".into());
            }
            let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(64 * 1024 * 1024);
            limits.max_image_width = Some(8000);
            limits.max_image_height = Some(8000);
            reader.limits(limits);
            reader.decode().map_err(|e| e.to_string())?;
            if let Some(expected) = args.internal_asset_hashes.get(&b.id) {
                if writing::hash(&bytes) != *expected {
                    return Err(format!(
                        "L’image {} a changé depuis le document original ; restauration du modèle refusée",
                        b.path
                    ));
                }
            }
            images.insert(b.id.clone(), bytes);
            assets.push(advanced::Asset {
                path: b.path.clone(),
                name: format!("image-{i}.png"),
            });
        }
    }
    let bytes = if args.raw {
        let sections: Vec<_> = model_blocks
            .iter()
            .map(|b| generate::Section {
                heading: if b.kind == "heading" {
                    b.text.clone()
                } else {
                    String::new()
                },
                text: if b.kind != "heading" {
                    b.text.clone()
                } else {
                    String::new()
                },
                bullets: b.items.clone(),
            })
            .collect();
        match ext.as_str() {
            "pdf" => generate::pdf(&args.title, &sections)?,
            "docx" => generate::docx(&args.title, &sections)?,
            "pptx" => generate::pptx(&args.title, &sections)?,
            _ => return Err("PDF, DOCX ou PPTX requis".into()),
        }
    } else {
        match ext.as_str() {
            "pdf" => {
                let mut data_blocks = serde_json::to_value(&content).map_err(|e| e.to_string())?;
                for (i, b) in content.iter().enumerate() {
                    if b.kind == "image" {
                        data_blocks[i]["asset_name"] = json!(format!("image-{i}.png"))
                    }
                }
                let logo = state::logo(&brand)?;
                let mut public_brand = brand.clone();
                public_brand.logo_base64.clear();
                let data = json!({"title":args.title,"style":style_label(&args.style),"brand":public_brand,"has_logo":logo.is_some(),"blocks":data_blocks,"cover":cover,"toc":toc,"sources":sources});
                let inline = logo
                    .map(|b| vec![("brand-logo.png".into(), b)])
                    .unwrap_or_default();
                let compiled = advanced::compile_typst_with_inline(
                    folders,
                    &args.folder,
                    TYPST,
                    &assets,
                    Some(&data),
                    &inline,
                )
                .await?;
                if compiled
                    .warnings
                    .to_ascii_lowercase()
                    .contains("missing glyph")
                {
                    return Err(format!(
                        "Caractère non couvert par les polices intégrées : {}",
                        compiled.warnings
                    ));
                }
                compiled.bytes
            }
            "docx" => docx(&args.title, &content, &brand, cover, toc, &sources, &images)?,
            "pptx" => {
                let mut slides_content = content.clone();
                if !sources.is_empty() {
                    slides_content.push(Block {
                        kind: "heading".into(),
                        text: "Sources".into(),
                        level: 1,
                        ..Default::default()
                    });
                    slides_content.push(Block {
                        kind: "list".into(),
                        items: sources.clone(),
                        ..Default::default()
                    });
                }
                super::slides::create(&args.title, &slides_content, &brand, &images)?
            }
            _ => return Err("Créer un PDF, DOCX ou PPTX ; Excel : write_spreadsheet".into()),
        }
    };
    let report = quality::inspect_bytes(&bytes, &ext)?;
    if report["errors"].as_array().is_some_and(|v| !v.is_empty()) {
        return Err(format!("Document refusé avant enregistrement : {report}"));
    }
    let mut result = writing::commit(folders, &args.folder, &args.path, &bytes, None)?;
    result["design"] = json!({"mode":if args.raw{"raw"}else{"professional"},"style":args.style,"brand":brand.id,"cover":cover,"table_of_contents":toc});
    result["quality"] = quality::compact(&report);
    result["sources"] = json!(sources);
    result["next_action"] = json!(if ext == "pdf" {
        "Appeler review_document pour vérifier visuellement les pages avant livraison."
    } else {
        "Inspecter les éléments ; vérifier le rendu final dans un lecteur Office."
    });
    args.blocks = std::mem::take(&mut model_blocks);
    args.sections.clear();
    let image_hashes: BTreeMap<_, _> = images
        .iter()
        .map(|(id, b)| (id.clone(), writing::hash(b)))
        .collect();
    let model =
        json!({"sha256":result["sha256"],"args":args,"identity":brand,"assets":image_hashes});
    let full = library::resolve(folders, &args.folder, &args.path)?;
    let model_path = state::root()?
        .join("models")
        .join(format!("{}.json", state::scope(&full)));
    if let Err(e) = state::write_json(&model_path, &model) {
        result["model_warning"] = json!(e)
    }
    Ok(result)
}

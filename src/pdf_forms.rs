use crate::{advanced, library::SharedFolder, writing};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt::Display,
    fs,
};
type R<T> = Result<T, String>;
fn e(error: impl Display) -> String {
    error.to_string()
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct InspectArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
#[derive(Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct FillArgs {
    #[serde(default)]
    pub folder: String,
    pub path: String,
    pub output_path: String,
    /// Empreinte source obtenue avec inspect_pdf_form.
    pub expected_sha256: String,
    /// Nom complet de champ -> texte, booléen pour case simple, nom d’option pour radio/liste.
    pub values: BTreeMap<String, Value>,
    /// Taille maximale ; réduction automatique si nécessaire. Défaut 12 pt, 4..36.
    pub font_size: Option<f32>,
}
#[derive(Clone)]
struct Field {
    id: ObjectId,
    name: String,
    kind: String,
    flags: i64,
    inherited: Dictionary,
    widgets: Vec<ObjectId>,
}
fn dict(doc: &Document, obj: &Object) -> R<Dictionary> {
    Ok(doc
        .dereference(obj)
        .map_err(e)?
        .1
        .as_dict()
        .map_err(e)?
        .clone())
}
fn array(doc: &Document, obj: &Object) -> R<Vec<Object>> {
    Ok(doc
        .dereference(obj)
        .map_err(e)?
        .1
        .as_array()
        .map_err(e)?
        .clone())
}
fn string(obj: &Object) -> String {
    match obj {
        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
        Object::String(_, _) => lopdf::decode_text_string(obj).unwrap_or_default(),
        _ => String::new(),
    }
}
fn value(obj: &Object) -> Value {
    match obj {
        Object::Array(a) => json!(a.iter().map(value).collect::<Vec<_>>()),
        Object::Boolean(b) => json!(b),
        Object::Integer(n) => json!(n),
        _ => json!(string(obj)),
    }
}
fn unicode(text: &str) -> Object {
    let bytes = std::iter::once(0xFEFFu16)
        .chain(text.encode_utf16())
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}
fn walk(
    doc: &Document,
    id: ObjectId,
    prefix: &str,
    inherited: &Dictionary,
    seen: &mut HashSet<ObjectId>,
    fields: &mut Vec<Field>,
    depth: usize,
) -> R<()> {
    if depth > 32 || seen.len() > 5000 || !seen.insert(id) {
        return Err("Arborescence AcroForm cyclique ou trop complexe".into());
    }
    let d = doc.get_dictionary(id).map_err(e)?;
    let mut inherited = inherited.clone();
    for key in [
        b"FT".as_slice(),
        b"Ff",
        b"Opt",
        b"MaxLen",
        b"V",
        b"Q",
        b"DA",
    ] {
        if let Ok(v) = d.get(key) {
            inherited.set(key.to_vec(), v.clone());
        }
    }
    let partial = d.get(b"T").ok().map(string).unwrap_or_default();
    let full = if partial.is_empty() {
        prefix.into()
    } else if prefix.is_empty() {
        partial
    } else {
        format!("{prefix}.{partial}")
    };
    let mut widgets = Vec::new();
    if d.get(b"Subtype")
        .is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"Widget"))
    {
        widgets.push(id);
    }
    let mut children = Vec::new();
    if let Ok(kids) = d.get(b"Kids") {
        for child in array(doc, kids)? {
            let child_id = child
                .as_reference()
                .map_err(|_| "Champ direct non pris en charge ; références PDF requises")?;
            let kd = doc.get_dictionary(child_id).map_err(e)?;
            let is_widget = kd
                .get(b"Subtype")
                .is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"Widget"));
            if is_widget && !kd.has(b"T") && !kd.has(b"Kids") {
                if kd.get(b"Parent").and_then(Object::as_reference).ok() != Some(id) {
                    return Err("Widget et champ canonique sans relation Parent cohérente".into());
                }
                widgets.push(child_id);
            } else {
                children.push(child_id);
            }
        }
    }
    if children.is_empty() {
        let kind = inherited.get(b"FT").ok().map(string).unwrap_or_default();
        if !kind.is_empty() {
            if full.is_empty() {
                return Err("Champ PDF sans nom".into());
            }
            fields.push(Field {
                id,
                name: full,
                kind,
                flags: inherited.get(b"Ff").and_then(Object::as_i64).unwrap_or(0),
                inherited,
                widgets,
            });
        }
    } else {
        for child in children {
            walk(doc, child, &full, &inherited, seen, fields, depth + 1)?;
        }
    }
    Ok(())
}
fn fields(doc: &Document) -> R<(Dictionary, Vec<Field>)> {
    if doc.is_encrypted() {
        return Err("PDF chiffré : fournir une copie déverrouillée".into());
    }
    let acro = doc
        .catalog()
        .map_err(e)?
        .get(b"AcroForm")
        .map_err(|_| "Ce PDF ne contient pas de formulaire AcroForm interactif")?;
    let acro = dict(doc, acro)?;
    if acro.has(b"XFA") {
        return Err("Formulaires XFA non pris en charge ; utiliser un AcroForm standard".into());
    }
    let root = array(doc, acro.get(b"Fields").map_err(e)?)?;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for field in root {
        walk(
            doc,
            field.as_reference().map_err(e)?,
            "",
            &Dictionary::new(),
            &mut seen,
            &mut out,
            0,
        )?;
    }
    let mut names = HashSet::new();
    let mut widgets = HashSet::new();
    for f in &out {
        if !names.insert(f.name.clone()) {
            return Err(format!("Nom AcroForm ambigu : {}", f.name));
        }
        for id in &f.widgets {
            if !widgets.insert(*id) {
                return Err("Widget partagé entre champs ambigus".into());
            }
        }
    }
    for (_, page) in doc.get_pages() {
        let d = doc.get_dictionary(page).map_err(e)?;
        if let Ok(annots) = d.get(b"Annots") {
            for annot in array(doc, annots)? {
                let ad = dict(doc, &annot)?;
                if ad
                    .get(b"Subtype")
                    .is_ok_and(|v| v.as_name().is_ok_and(|n| n == b"Widget"))
                    && !annot.as_reference().is_ok_and(|id| widgets.contains(&id))
                {
                    return Err("Widget de page absent du véritable AcroForm/Fields ; réparation explicite nécessaire".into());
                }
            }
        }
    }
    Ok((acro, out))
}
fn options(doc: &Document, field: &Field) -> R<Vec<(String, String)>> {
    let mut out = Vec::new();
    if let Ok(opt) = field.inherited.get(b"Opt") {
        for option in array(doc, opt)? {
            if let Object::Array(pair) = option {
                if pair.len() != 2 {
                    return Err("Option de liste invalide".into());
                }
                out.push((string(&pair[0]), string(&pair[1])));
            } else {
                let text = string(&option);
                out.push((text.clone(), text));
            }
        }
    }
    Ok(out)
}
fn states(doc: &Document, id: ObjectId) -> R<Vec<Vec<u8>>> {
    let d = doc.get_dictionary(id).map_err(e)?;
    let ap = dict(
        doc,
        d.get(b"AP").map_err(|_| "Apparence de bouton absente")?,
    )?;
    let normal = dict(doc, ap.get(b"N").map_err(e)?)?;
    let mut states = Vec::new();
    for (name, obj) in normal.iter() {
        let stream = doc.dereference(obj).map_err(e)?.1.as_stream().map_err(e)?;
        if stream.content.is_empty() {
            return Err("Apparence de bouton vide".into());
        }
        if name != b"Off" {
            states.push(name.clone());
        }
    }
    Ok(states)
}
fn load(folders: &[SharedFolder], folder: &str, path: &str) -> R<(Document, Vec<u8>)> {
    let bytes = fs::read(advanced::input(folders, folder, path)?).map_err(e)?;
    let doc = Document::load_mem(&bytes).map_err(e)?;
    Ok((doc, bytes))
}
#[cfg(test)]
pub fn inspect(folders: &[SharedFolder], folder: &str, path: &str) -> R<Value> {
    inspect_window(folders, folder, path, 0, 50)
}
pub fn inspect_window(
    folders: &[SharedFolder],
    folder: &str,
    path: &str,
    offset: usize,
    limit: usize,
) -> R<Value> {
    let (doc, bytes) = load(folders, folder, path)?;
    let (_, fields) = fields(&doc)?;
    let count = fields.len();
    let mut result = Vec::new();
    for f in fields.into_iter().skip(offset).take(limit.clamp(1, 100)) {
        let mut v = json!({"name":f.name,"type":f.kind,"read_only":f.flags&1!=0,"widgets":f.widgets.len(),"value":f.inherited.get(b"V").ok().map(value),"max_length":f.inherited.get(b"MaxLen").ok().and_then(|v|v.as_i64().ok())});
        if f.kind == "Ch" {
            v["options"] = json!(
                options(&doc, &f)?
                    .into_iter()
                    .map(|(export, label)| json!({"value":export,"label":label}))
                    .collect::<Vec<_>>()
            );
        } else if f.kind == "Btn" {
            v["radio"] = json!(f.flags & (1 << 15) != 0);
            let states = f
                .widgets
                .iter()
                .map(|id| {
                    states(&doc, *id).map(|s| {
                        s.iter()
                            .map(|b| String::from_utf8_lossy(b).into_owned())
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<R<Vec<_>>>()?;
            v["export_states_by_widget"] = json!(states);
        }
        fn compact(value: &mut Value) {
            match value {
                Value::String(s) => {
                    if s.chars().count() > 2000 {
                        *s = s.chars().take(2000).collect();
                    }
                }
                Value::Array(a) => {
                    a.truncate(200);
                    for v in a {
                        compact(v);
                    }
                }
                Value::Object(o) => {
                    for v in o.values_mut() {
                        compact(v)
                    }
                }
                _ => {}
            }
        }
        compact(&mut v);
        if result
            .iter()
            .map(|v: &Value| v.to_string().len())
            .sum::<usize>()
            + v.to_string().len()
            > 128000
        {
            break;
        }
        result.push(v);
    }
    let next = offset.saturating_add(result.len());
    Ok(
        json!({"sha256":writing::hash(&bytes),"fields":result,"total":count,"truncated":next<count,"next_offset":(next<count).then_some(next),"value_preview_characters":2000,"interactive":true}),
    )
}
fn font(doc: &mut Document, texts: &[String]) -> R<ObjectId> {
    let bytes = include_bytes!("../assets/fonts/Lato-Regular.ttf");
    let face = ttf_parser::Face::parse(bytes, 0).map_err(e)?;
    let units = face.units_per_em() as f32;
    let mut glyphs = BTreeMap::<u16, char>::new();
    for c in texts
        .iter()
        .flat_map(|t| t.chars())
        .filter(|c| !c.is_control())
    {
        let gid = face
            .glyph_index(c)
            .ok_or_else(|| format!("Caractère {c:?} absent de Lato : formulaire non modifié"))?;
        glyphs.entry(gid.0).or_insert(c);
    }
    let stream = doc.add_object(Stream::new(
        dictionary! {"Length1"=>bytes.len() as i64},
        bytes.to_vec(),
    ));
    let bbox = face.global_bounding_box();
    let scale = |n: i16| n as f32 * 1000.0 / units;
    let descriptor=doc.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"Lato-Regular","Flags"=>32,"FontBBox"=>vec![Object::Real(scale(bbox.x_min)),Object::Real(scale(bbox.y_min)),Object::Real(scale(bbox.x_max)),Object::Real(scale(bbox.y_max))],"ItalicAngle"=>0,"Ascent"=>scale(face.ascender()),"Descent"=>scale(face.descender()),"CapHeight"=>scale(face.capital_height().unwrap_or(face.ascender())),"StemV"=>80,"FontFile2"=>stream});
    let mut widths = Vec::new();
    for id in glyphs.keys() {
        widths.push(Object::Integer(*id as i64));
        widths.push(Object::Array(vec![Object::Real(
            face.glyph_hor_advance(ttf_parser::GlyphId(*id))
                .unwrap_or(1000) as f32
                * 1000.0
                / units,
        )]));
    }
    let cid=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"Lato-Regular","CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"CIDToGIDMap"=>"Identity","DW"=>1000,"W"=>Object::Array(widths)});
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /MCPUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let pairs = glyphs.into_iter().collect::<Vec<_>>();
    for chunk in pairs.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (gid, c) in chunk {
            let unicode = c
                .encode_utf16(&mut [0; 2])
                .iter()
                .map(|v| format!("{v:04X}"))
                .collect::<String>();
            cmap.push_str(&format!("<{gid:04X}> <{unicode}>\n"));
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend");
    let to_unicode = doc.add_object(Stream::new(Dictionary::new(), cmap.into_bytes()));
    Ok(doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"Lato-Regular","Encoding"=>"Identity-H","DescendantFonts"=>vec![Object::Reference(cid)],"ToUnicode"=>to_unicode}))
}
fn appearance(
    doc: &mut Document,
    widget: ObjectId,
    text: &str,
    multiline: bool,
    font: ObjectId,
    max_size: f32,
    align: i64,
) -> R<ObjectId> {
    let d = doc.get_dictionary(widget).map_err(e)?.clone();
    let rect = array(doc, d.get(b"Rect").map_err(|_| "Widget sans rectangle")?)?;
    if rect.len() != 4 {
        return Err("Rectangle de champ invalide".into());
    }
    let x1 = rect[0].as_float().map_err(e)?;
    let y1 = rect[1].as_float().map_err(e)?;
    let w = (rect[2].as_float().map_err(e)? - x1).abs();
    let h = (rect[3].as_float().map_err(e)? - y1).abs();
    if !w.is_finite() || !h.is_finite() || w < 8.0 || h < 8.0 || w > 3000.0 || h > 3000.0 {
        return Err("Taille du champ non prise en charge".into());
    }
    if d.get(b"MK")
        .ok()
        .and_then(|o| dict(doc, o).ok())
        .and_then(|m| m.get(b"R").ok().and_then(|v| v.as_i64().ok()))
        .unwrap_or(0)
        != 0
    {
        return Err("Champ avec rotation : remplissage visuel non pris en charge".into());
    }
    if !multiline && text.contains('\n') {
        return Err("Champ simple : une seule ligne attendue".into());
    }
    let face = ttf_parser::Face::parse(include_bytes!("../assets/fonts/Lato-Regular.ttf"), 0)
        .map_err(e)?;
    let unit = face.units_per_em() as f32;
    let measure = |text: &str, size: f32| {
        text.chars()
            .map(|c| {
                face.glyph_index(c)
                    .and_then(|g| face.glyph_hor_advance(g))
                    .unwrap_or(0) as f32
                    * size
                    / unit
            })
            .sum::<f32>()
    };
    let mut size = max_size;
    let lines = loop {
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for c in paragraph.chars() {
                if multiline && !line.is_empty() && measure(&format!("{line}{c}"), size) > w - 8.0 {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(c);
            }
            lines.push(line);
        }
        let line_height = (face.ascender() as f32 - face.descender() as f32) * size / unit * 1.05;
        if lines.iter().all(|l| measure(l, size) <= w - 8.0)
            && line_height * lines.len() as f32 <= h - 6.0
        {
            break lines;
        }
        size -= 0.5;
        if size < 4.0 {
            return Err(
                "Le texte ne tient pas dans le champ, même à 4 pt ; raccourcir la valeur".into(),
            );
        }
    };
    let line_height = (face.ascender() as f32 - face.descender() as f32) * size / unit * 1.05;
    let mut y = h - 3.0 - face.ascender() as f32 * size / unit;
    let mut content = format!(
        "q 1 1 1 rg 0 0 {w} {h} re f 0.6 0.6 0.6 RG 0.5 w 0.25 0.25 {} {} re S 2 2 {} {} re W n BT /F0 {size} Tf 0 g\n",
        w - 0.5,
        h - 0.5,
        w - 4.0,
        h - 4.0
    );
    for line in lines {
        let width = measure(&line, size);
        let x = match align {
            1 => (w - width) / 2.0,
            2 => w - 4.0 - width,
            _ => 4.0,
        };
        let glyphs = line
            .chars()
            .map(|c| {
                face.glyph_index(c)
                    .map(|g| format!("{:04X}", g.0))
                    .ok_or_else(|| format!("Glyph manquant : {c:?}"))
            })
            .collect::<R<Vec<_>>>()?
            .join("");
        content.push_str(&format!("1 0 0 1 {x} {y} Tm <{glyphs}> Tj\n"));
        y -= line_height;
    }
    content.push_str("ET Q");
    Ok(doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![Object::Integer(0),Object::Integer(0),Object::Real(w),Object::Real(h)],"Resources"=>dictionary!{"Font"=>dictionary!{"F0"=>font}}},content.into_bytes())))
}
fn fill_bytes(mut doc: Document, args: &FillArgs) -> R<Vec<u8>> {
    let (mut acro, fields) = fields(&doc)?;
    if doc.objects.values().any(|o| {
        o.as_dict()
            .is_ok_and(|d| d.has(b"ByteRange") && d.has(b"Contents"))
    }) {
        return Err("PDF signé : modification refusée pour préserver la signature".into());
    }
    if fields.iter().any(|f| {
        f.kind == "Sig"
            && f.inherited
                .get(b"V")
                .is_ok_and(|v| !matches!(v, Object::Null))
    }) {
        return Err("PDF signé : modification refusée pour préserver la signature".into());
    }
    if args.values.is_empty()
        || args.values.len() > 200
        || serde_json::to_vec(&args.values).map_err(e)?.len() > 1_000_000
    {
        return Err("Fournir de 1 à 200 valeurs".into());
    }
    let font_size = args.font_size.unwrap_or(12.0);
    if !font_size.is_finite() || !(4.0..=36.0).contains(&font_size) {
        return Err("font_size attendu entre 4 et 36 pt".into());
    }
    let by_name: HashMap<_, _> = fields.iter().map(|f| (f.name.as_str(), f)).collect();
    let mut texts = Vec::new();
    let mut pending = Vec::new();
    for (name, v) in &args.values {
        let f = *by_name
            .get(name.as_str())
            .ok_or_else(|| format!("Champ inconnu : {name}. Appeler inspect_pdf_form."))?;
        if f.flags & 1 != 0 {
            return Err(format!("Champ en lecture seule : {name}"));
        }
        match f.kind.as_str() {
            "Tx" => {
                if f.flags & ((1 << 20) | (1 << 24) | (1 << 25)) != 0 {
                    return Err("Champ texte fichier, peigne ou enrichi non pris en charge".into());
                }
                let text = v
                    .as_str()
                    .ok_or("Champ texte : chaîne attendue")?
                    .replace("\r\n", "\n")
                    .replace('\r', "\n")
                    .replace('\t', "    ");
                if text.len() > 32000 || text.chars().any(|c| c.is_control() && c != '\n') {
                    return Err("Texte de champ trop long ou invalide".into());
                }
                if f.inherited
                    .get(b"MaxLen")
                    .and_then(Object::as_i64)
                    .is_ok_and(|n| text.chars().count() as i64 > n)
                {
                    return Err(format!("MaxLen dépassé : {name}"));
                }
                let display = if f.flags & (1 << 13) != 0 {
                    "•".repeat(text.chars().count())
                } else {
                    text.clone()
                };
                texts.push(display.clone());
                pending.push((f, unicode(&text), display));
            }
            "Ch" => {
                let opts = options(&doc, f)?;
                let selected = if let Some(s) = v.as_str() {
                    vec![s.to_owned()]
                } else if let Some(list) = v.as_array() {
                    list.iter()
                        .map(|v| {
                            v.as_str()
                                .map(str::to_owned)
                                .ok_or("Liste de chaînes attendue")
                        })
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    return Err("Choix : chaîne ou tableau de chaînes attendu".into());
                };
                if selected.is_empty()
                    || selected.len() > 100
                    || selected.len() > 1 && f.flags & (1 << 21) == 0
                {
                    return Err("Sélection multiple non autorisée".into());
                }
                let editable = f.flags & (1 << 18) != 0;
                let mut labels = Vec::new();
                for selected in &selected {
                    let found = opts.iter().find(|(export, _)| export == selected);
                    if !editable && found.is_none() {
                        return Err(format!("Option inconnue : {selected}"));
                    }
                    labels.push(
                        found
                            .map(|(_, label)| label.clone())
                            .unwrap_or_else(|| selected.clone()),
                    );
                }
                let display = labels.join("\n");
                texts.push(display.clone());
                let val = if selected.len() == 1 {
                    unicode(&selected[0])
                } else {
                    Object::Array(selected.iter().map(|s| unicode(s)).collect())
                };
                pending.push((f, val, display));
            }
            "Btn" => {
                if f.flags & (1 << 16) != 0 {
                    return Err("Boutons poussoirs non remplissables".into());
                }
                let mut available = BTreeSet::new();
                for widget in &f.widgets {
                    available.extend(states(&doc, *widget)?);
                }
                let target = if let Some(b) = v.as_bool() {
                    if b {
                        if available.len() != 1 {
                            return Err(
                                "Radio ou états multiples : fournir le nom d’export exact".into()
                            );
                        }
                        available.iter().next().unwrap().clone()
                    } else {
                        b"Off".to_vec()
                    }
                } else {
                    v.as_str()
                        .ok_or("Bouton : booléen ou nom d’état attendu")?
                        .as_bytes()
                        .to_vec()
                };
                if target != b"Off" && !available.contains(&target) {
                    return Err("État de bouton inconnu ; consulter inspect_pdf_form".into());
                }
                if target == b"Off" && f.flags & (1 << 14) != 0 {
                    return Err("Ce groupe radio ne peut pas être désélectionné".into());
                }
                pending.push((f, Object::Name(target), String::new()));
            }
            _ => {
                return Err(format!(
                    "Champ {} de type {} non remplissable",
                    f.name, f.kind
                ));
            }
        }
    }
    let font = if texts.is_empty() {
        None
    } else {
        Some(font(&mut doc, &texts)?)
    };
    for (f, new_value, display) in pending {
        if f.kind == "Btn" {
            let target = new_value.as_name().map_err(e)?.to_vec();
            for widget in &f.widgets {
                let state = if target != b"Off" && states(&doc, *widget)?.contains(&target) {
                    target.clone()
                } else {
                    b"Off".to_vec()
                };
                let d = doc.get_dictionary_mut(*widget).map_err(e)?;
                d.set("AS", Object::Name(state));
                d.set("V", new_value.clone());
            }
        } else {
            for widget in &f.widgets {
                let ap = appearance(
                    &mut doc,
                    *widget,
                    &display,
                    f.kind == "Ch" || f.flags & (1 << 12) != 0,
                    font.unwrap(),
                    font_size,
                    f.inherited.get(b"Q").and_then(Object::as_i64).unwrap_or(0),
                )?;
                let d = doc.get_dictionary_mut(*widget).map_err(e)?;
                d.set("AP", dictionary! {"N"=>ap});
                d.set("V", new_value.clone());
            }
            let d = doc.get_dictionary_mut(f.id).map_err(e)?;
            d.set(
                "DA",
                Object::string_literal(format!("/MCPFont {font_size} Tf 0 g")),
            );
            d.remove(b"RV");
            if f.kind == "Ch" {
                d.remove(b"I");
            }
        }
        doc.get_dictionary_mut(f.id).map_err(e)?.set("V", new_value);
    }
    if let Some(font) = font {
        let mut dr = acro
            .get(b"DR")
            .ok()
            .map(|d| dict(&doc, d))
            .transpose()?
            .unwrap_or_default();
        let mut fonts = dr
            .get(b"Font")
            .ok()
            .map(|d| dict(&doc, d))
            .transpose()?
            .unwrap_or_default();
        fonts.set("MCPFont", font);
        dr.set("Font", fonts);
        acro.set("DR", dr);
    }
    acro.set("NeedAppearances", false);
    doc.catalog_mut().map_err(e)?.set("AcroForm", acro);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).map_err(e)?;
    // Reopen and check canonical values, widget values and appearances, not merely a PNG.
    let reopened = Document::load_mem(&bytes).map_err(e)?;
    let (_, written) = self::fields(&reopened)?;
    for name in args.values.keys() {
        let field = written
            .iter()
            .find(|f| &f.name == name)
            .ok_or("Champ disparu après écriture")?;
        let expected = field.inherited.get(b"V").map_err(e)?;
        for id in &field.widgets {
            let d = reopened.get_dictionary(*id).map_err(e)?;
            if d.get(b"V").ok() != Some(expected) {
                return Err("Valeur widget/canonique incohérente après sauvegarde".into());
            }
            let ap = dict(&reopened, d.get(b"AP").map_err(e)?)?;
            if field.kind == "Btn" {
                let _ = states(&reopened, *id)?;
            } else if reopened
                .dereference(ap.get(b"N").map_err(e)?)
                .map_err(e)?
                .1
                .as_stream()
                .map_err(e)?
                .content
                .is_empty()
            {
                return Err("Apparence de formulaire vide".into());
            }
        }
    }
    Ok(bytes)
}
pub fn fill(folders: &[SharedFolder], args: FillArgs) -> R<Value> {
    writing::destination(folders, &args.folder, &args.output_path)?;
    if !args.output_path.to_lowercase().ends_with(".pdf") {
        return Err("Sortie PDF requise".into());
    }
    let (doc, bytes) = load(folders, &args.folder, &args.path)?;
    if writing::hash(&bytes) != args.expected_sha256 {
        return Err("La source a changé : rappeler inspect_pdf_form".into());
    }
    let bytes = fill_bytes(doc, &args)?;
    let mut result = writing::commit(folders, &args.folder, &args.output_path, &bytes, None)?;
    result["filled_fields"] = json!(args.values.keys().collect::<Vec<_>>());
    result["interactive"] = json!(true);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages = doc.new_object_id();
        let page = doc.new_object_id();
        let text = doc.new_object_id();
        let widget = doc.new_object_id();
        let check = doc.new_object_id();
        let check_widget = doc.new_object_id();
        let choice = doc.new_object_id();
        let choice_widget = doc.new_object_id();
        let off=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()]},b"q 1 1 1 rg 0 0 20 20 re f 0 G 1 w 1 1 18 18 re S Q".to_vec()));
        let on=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()]},b"q 1 1 1 rg 0 0 20 20 re f 0 G 1 w 1 1 18 18 re S 2 w 4 10 m 8 5 l 16 16 l S Q".to_vec()));
        let blank=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),300.into(),32.into()]},b"q 1 1 1 rg 0 0 300 32 re f Q".to_vec()));
        doc.objects.insert(text,Object::Dictionary(dictionary!{"FT"=>"Tx","T"=>Object::string_literal("client.name"),"V"=>Object::string_literal("old"),"MaxLen"=>100,"Kids"=>vec![Object::Reference(widget)]}));
        doc.objects.insert(widget,Object::Dictionary(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>text,"P"=>page,"F"=>4,"Rect"=>vec![60.into(),650.into(),360.into(),682.into()],"AP"=>dictionary!{"N"=>blank}}));
        doc.objects.insert(check,Object::Dictionary(dictionary!{"FT"=>"Btn","T"=>Object::string_literal("approved"),"V"=>Object::Name(b"Off".to_vec()),"Kids"=>vec![Object::Reference(check_widget)]}));
        doc.objects.insert(check_widget,Object::Dictionary(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>check,"P"=>page,"F"=>4,"Rect"=>vec![60.into(),580.into(),80.into(),600.into()],"AS"=>"Off","AP"=>dictionary!{"N"=>dictionary!{"Off"=>off,"Yes"=>on}}}));
        doc.objects.insert(choice,Object::Dictionary(dictionary!{"FT"=>"Ch","T"=>Object::string_literal("department"),"Opt"=>vec![Object::string_literal("Support"),Object::string_literal("Finance")],"Kids"=>vec![Object::Reference(choice_widget)]}));
        doc.objects.insert(choice_widget,Object::Dictionary(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>choice,"P"=>page,"F"=>4,"Rect"=>vec![60.into(),500.into(),360.into(),532.into()],"AP"=>dictionary!{"N"=>blank}}));
        let font =
            doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let content=doc.add_object(Stream::new(Dictionary::new(),b"BT /F1 24 Tf 60 750 Td (AcroForm - MCP Win v0.4) Tj /F1 12 Tf 0 -50 Td (Client name) Tj 0 -80 Td (Approved) Tj 0 -80 Td (Department) Tj ET".to_vec()));
        doc.objects.insert(page,Object::Dictionary(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),595.into(),842.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content,"Annots"=>vec![Object::Reference(widget),Object::Reference(check_widget),Object::Reference(choice_widget)]}));
        doc.objects.insert(
            pages,
            Object::Dictionary(
                dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
            ),
        );
        let acro=doc.add_object(dictionary!{"Fields"=>vec![Object::Reference(text),Object::Reference(check),Object::Reference(choice)],"NeedAppearances"=>false});
        let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>acro});
        doc.trailer.set("Root", root);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }
    #[test]
    fn fills_canonical_fields_widgets_appearances_and_preserves_interactivity() {
        let temp = tempfile::tempdir().unwrap();
        let source = fixture();
        fs::write(temp.path().join("form.pdf"), &source).unwrap();
        let folders = [SharedFolder {
            id: "test".into(),
            path: temp.path().into(),
            writable: true,
        }];
        let report = inspect(&folders, "", "form.pdf").unwrap();
        assert_eq!(report["total"], 3);
        let values = BTreeMap::from([
            ("client.name".into(), json!("Élodie Noël – été 2026")),
            ("approved".into(), json!(true)),
            ("department".into(), json!("Support")),
        ]);
        let args = FillArgs {
            folder: "".into(),
            path: "form.pdf".into(),
            output_path: "filled.pdf".into(),
            expected_sha256: writing::hash(&source),
            values,
            font_size: None,
        };
        fill(&folders, args).unwrap();
        assert_eq!(fs::read(temp.path().join("form.pdf")).unwrap(), source);
        let bytes = fs::read(temp.path().join("filled.pdf")).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        let (acro, fields) = fields(&doc).unwrap();
        assert_eq!(
            acro.get(b"NeedAppearances").unwrap(),
            &Object::Boolean(false)
        );
        let field = fields.iter().find(|f| f.name == "client.name").unwrap();
        assert_eq!(
            string(field.inherited.get(b"V").unwrap()),
            "Élodie Noël – été 2026"
        );
        assert_ne!(field.id, field.widgets[0]);
        let w = doc.get_dictionary(field.widgets[0]).unwrap();
        assert_eq!(w.get(b"Parent").unwrap().as_reference().unwrap(), field.id);
        assert!(w.has(b"AP"));
        let check = fields.iter().find(|f| f.name == "approved").unwrap();
        assert_eq!(string(check.inherited.get(b"V").unwrap()), "Yes");
        assert_eq!(
            doc.get_dictionary(check.widgets[0])
                .unwrap()
                .get(b"AS")
                .unwrap()
                .as_name()
                .unwrap(),
            b"Yes"
        );
        // Generic PDF text extractors omit widget appearances; verify the field data and
        // embedded Unicode map directly, then render the interactive widget appearances.
        assert!(doc.objects.values().any(|o| {
            o.as_stream()
                .is_ok_and(|s| String::from_utf8_lossy(&s.content).contains("00C9"))
        }));
        let qa = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/qa-v04");
        fs::create_dir_all(&qa).unwrap();
        fs::write(qa.join("filled-form.pdf"), &bytes).unwrap();
        let (image, _) = advanced::pdf_image(&folders, "", "filled.pdf", 1, 1200).unwrap();
        fs::write(qa.join("filled-form.png"), advanced::png(&image).unwrap()).unwrap();
        assert!(
            crate::pdf_ops::combine(
                &folders,
                "",
                &[crate::pdf_ops::PdfInput {
                    path: "filled.pdf".into(),
                    pages: None
                }],
                "must-refuse.pdf",
                false
            )
            .is_err()
        );
        crate::pdf_ops::combine(
            &folders,
            "",
            &[crate::pdf_ops::PdfInput {
                path: "filled.pdf".into(),
                pages: None,
            }],
            "flat.pdf",
            true,
        )
        .unwrap();
        let flat = Document::load(temp.path().join("flat.pdf")).unwrap();
        assert!(!flat.catalog().unwrap().has(b"AcroForm"));
        let (flat_image, _) = advanced::pdf_image(&folders, "", "flat.pdf", 1, 1200).unwrap();
        let a = image.to_rgb8();
        let b = flat_image.to_rgb8();
        assert_eq!(a.dimensions(), b.dimensions());
        let delta = a
            .as_raw()
            .iter()
            .zip(b.as_raw())
            .map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs() as u64)
            .sum::<u64>() as f64
            / a.as_raw().len() as f64;
        assert!(
            delta < 1.0,
            "Form appearance lost while flattening: mean pixel difference {delta}"
        );
    }
    #[test]
    fn rejects_orphan_widgets_wrong_values_and_signed_forms() {
        let doc = Document::load_mem(&fixture()).unwrap();
        let (_, fields) = fields(&doc).unwrap();
        let text = fields.iter().find(|f| f.kind == "Tx").unwrap();
        let mut broken = doc.clone();
        broken.get_dictionary_mut(text.id).unwrap().remove(b"Kids");
        assert!(self::fields(&broken).is_err());
        let args = FillArgs {
            folder: "".into(),
            path: "".into(),
            output_path: "".into(),
            expected_sha256: "".into(),
            values: BTreeMap::from([("department".into(), json!("Unknown"))]),
            font_size: None,
        };
        let mut signed = doc.clone();
        let signature=signed.add_object(dictionary!{"Type"=>"Sig","ByteRange"=>vec![0.into(),1.into(),2.into(),3.into()],"Contents"=>Object::string_literal("fake-signature-for-refusal-test")});
        signed
            .get_dictionary_mut(signature)
            .unwrap()
            .set("Filter", "Adobe.PPKLite");
        let valid = FillArgs {
            values: BTreeMap::from([("client.name".into(), json!("Test"))]),
            ..args.clone()
        };
        assert!(fill_bytes(signed, &valid).unwrap_err().contains("signé"));
        assert!(fill_bytes(doc, &args).is_err());
    }
}

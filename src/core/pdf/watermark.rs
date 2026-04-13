use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, dictionary};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::core::path::{has_docx_extension, has_pdf_extension};
use crate::types::{PdfToolError, Result};

const DOCX_REL_TYPE_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const DOCX_REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const DOCX_HEADER_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";

fn prepend_page_contents(doc: &mut Document, page_id: ObjectId, content: Vec<u8>) -> Result<()> {
    let page = doc
        .get_dictionary(page_id)
        .map_err(|e| PdfToolError::new(format!("Failed to read page dictionary: {e}")))?;
    let mut current_content_list: Vec<Object> = match page.get(b"Contents") {
        Ok(Object::Reference(id)) => vec![Object::Reference(*id)],
        Ok(Object::Array(arr)) => arr.clone(),
        _ => vec![],
    };

    let content_object_id = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content)));
    current_content_list.insert(0, Object::Reference(content_object_id));

    let page_mut = doc
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(|e| PdfToolError::new(format!("Failed to update page contents: {e}")))?;
    page_mut.set("Contents", current_content_list);
    Ok(())
}

fn ensure_font_in_page_resources(
    doc: &mut Document,
    page_id: ObjectId,
    font_name: &str,
    font_id: ObjectId,
) -> Result<()> {
    let font_ref_id = {
        let resources_obj = doc
            .get_or_create_resources(page_id)
            .map_err(|e| PdfToolError::new(format!("Failed to get page resources: {e}")))?;
        let resources = resources_obj
            .as_dict_mut()
            .map_err(|e| PdfToolError::new(format!("Invalid resources dictionary: {e}")))?;

        if !resources.has(b"Font") {
            resources.set("Font", Dictionary::new());
        }

        let fonts_obj = resources
            .get_mut(b"Font")
            .map_err(|e| PdfToolError::new(format!("Failed to get font resources: {e}")))?;
        if let Object::Reference(id) = fonts_obj {
            Some(*id)
        } else {
            None
        }
    };

    if let Some(font_dict_id) = font_ref_id {
        let fonts = doc
            .get_object_mut(font_dict_id)
            .and_then(Object::as_dict_mut)
            .map_err(|e| PdfToolError::new(format!("Invalid font dictionary reference: {e}")))?;
        fonts.set(font_name, Object::Reference(font_id));
    } else {
        let resources_obj = doc
            .get_or_create_resources(page_id)
            .map_err(|e| PdfToolError::new(format!("Failed to get page resources: {e}")))?;
        let resources = resources_obj
            .as_dict_mut()
            .map_err(|e| PdfToolError::new(format!("Invalid resources dictionary: {e}")))?;
        let fonts = resources
            .get_mut(b"Font")
            .and_then(Object::as_dict_mut)
            .map_err(|e| PdfToolError::new(format!("Invalid font dictionary: {e}")))?;
        fonts.set(font_name, Object::Reference(font_id));
    }

    Ok(())
}

fn page_dimensions(doc: &Document, page_id: ObjectId) -> (f32, f32) {
    let default_size = (595.0f32, 842.0f32);
    let Ok(page) = doc.get_dictionary(page_id) else {
        return default_size;
    };
    let Ok(media_box) = page.get(b"MediaBox").and_then(Object::as_array) else {
        return default_size;
    };
    if media_box.len() != 4 {
        return default_size;
    }

    let x0 = media_box[0].as_float().unwrap_or(0.0);
    let y0 = media_box[1].as_float().unwrap_or(0.0);
    let x1 = media_box[2].as_float().unwrap_or(default_size.0);
    let y1 = media_box[3].as_float().unwrap_or(default_size.1);

    let width = (x1 - x0).abs();
    let height = (y1 - y0).abs();
    if width > 1.0 && height > 1.0 {
        (width, height)
    } else {
        default_size
    }
}

fn apply_pdf_watermark(input_path: &Path, output_path: &Path, watermark_text: &str) -> Result<()> {
    let mut doc = Document::load(input_path)
        .map_err(|e| PdfToolError::new(format!("Failed to read document: {e}")))?;
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let pages = doc.get_pages();
    for (_, page_id) in pages {
        ensure_font_in_page_resources(&mut doc, page_id, "FWM", font_id)?;

        let (width, height) = page_dimensions(&doc, page_id);
        let angle = 35.0f32.to_radians();
        let cos = angle.cos();
        let sin = angle.sin();

        let font_size = ((width.min(height) * 0.12).max(28.0)).min(96.0);
        let x = width * 0.18;
        let y = height * 0.45;

        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new("g", vec![0.85.into()]),
                Operation::new(
                    "cm",
                    vec![
                        cos.into(),
                        sin.into(),
                        (-sin).into(),
                        cos.into(),
                        x.into(),
                        y.into(),
                    ],
                ),
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["FWM".into(), font_size.into()]),
                Operation::new("Td", vec![0.into(), 0.into()]),
                Operation::new("Tj", vec![Object::string_literal(watermark_text)]),
                Operation::new("ET", vec![]),
                Operation::new("Q", vec![]),
            ],
        };

        let encoded = content
            .encode()
            .map_err(|e| PdfToolError::new(format!("Failed to encode watermark content: {e}")))?;
        prepend_page_contents(&mut doc, page_id, encoded)?;
    }

    doc.compress();
    doc.save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save watermarked output: {e}")))?;
    Ok(())
}

fn read_zip_entries(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let file = fs::File::open(path)
        .map_err(|e| PdfToolError::new(format!("Failed to open DOCX '{}': {e}", path.display())))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| PdfToolError::new(format!("Invalid DOCX archive: {e}")))?;

    let mut entries = BTreeMap::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PdfToolError::new(format!("Failed to read DOCX entry: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| {
            PdfToolError::new(format!("Failed to read DOCX entry '{}': {e}", entry.name()))
        })?;
        entries.insert(entry.name().to_string(), bytes);
    }

    Ok(entries)
}

fn write_zip_entries(path: &Path, entries: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let file = fs::File::create(path).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to create output DOCX '{}': {e}",
            path.display()
        ))
    })?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (name, data) in entries {
        writer
            .start_file(name, options)
            .map_err(|e| PdfToolError::new(format!("Failed to write DOCX entry '{name}': {e}")))?;
        writer.write_all(data).map_err(|e| {
            PdfToolError::new(format!("Failed to write DOCX content for '{name}': {e}"))
        })?;
    }

    writer
        .finish()
        .map_err(|e| PdfToolError::new(format!("Failed to finalize DOCX: {e}")))?;
    Ok(())
}

fn read_xml_entry(entries: &BTreeMap<String, Vec<u8>>, path: &str) -> Result<String> {
    let bytes = entries
        .get(path)
        .ok_or_else(|| PdfToolError::new(format!("Missing DOCX entry: '{path}'")))?;
    String::from_utf8(bytes.clone())
        .map_err(|_| PdfToolError::new(format!("DOCX entry '{path}' is not valid UTF-8 XML")))
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_docx_watermark_header_xml(text: &str) -> String {
    let escaped = escape_xml_text(text);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:p>
    <w:pPr>
      <w:jc w:val="center"/>
    </w:pPr>
    <w:r>
      <w:rPr>
        <w:color w:val="CFCFCF"/>
        <w:sz w:val="96"/>
        <w:szCs w:val="96"/>
        <w:b/>
      </w:rPr>
      <w:t>{escaped}</w:t>
    </w:r>
  </w:p>
</w:hdr>"#
    )
}

fn ensure_docx_content_type_override(
    content_types_xml: &mut String,
    part_name: &str,
) -> Result<()> {
    if content_types_xml.contains(part_name) {
        return Ok(());
    }

    let override_entry =
        format!(r#"<Override PartName="{part_name}" ContentType="{DOCX_HEADER_CONTENT_TYPE}"/>"#);
    if let Some(pos) = content_types_xml.find("</Types>") {
        content_types_xml.insert_str(pos, &override_entry);
        Ok(())
    } else {
        Err(PdfToolError::new(
            "Invalid [Content_Types].xml: missing </Types>.",
        ))
    }
}

fn extract_xml_attr(tag: &str, attr_name: &str) -> Option<String> {
    let needle = format!(r#"{attr_name}=""#);
    let start = tag.find(&needle)?;
    let rest = &tag[start + needle.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn ensure_docx_header_relationship(rels_xml: &mut String, target: &str) -> Result<String> {
    let mut cursor = 0usize;
    while let Some(rel_start_rel) = rels_xml[cursor..].find("<Relationship") {
        let rel_start = cursor + rel_start_rel;
        let Some(rel_end_rel) = rels_xml[rel_start..].find('>') else {
            break;
        };
        let rel_end = rel_start + rel_end_rel + 1;
        let tag = &rels_xml[rel_start..rel_end];
        if tag.contains(&format!(r#"Target="{target}""#)) && tag.contains(DOCX_REL_TYPE_HEADER) {
            if let Some(existing_id) = extract_xml_attr(tag, "Id") {
                return Ok(existing_id);
            }
        }
        cursor = rel_end;
    }

    let mut max_id = 0u32;
    let mut search = 0usize;
    while let Some(idx_rel) = rels_xml[search..].find(r#"Id="rId"#) {
        let start = search + idx_rel + r#"Id="rId"#.len();
        let digits = rels_xml[start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>();
        if let Ok(num) = digits.parse::<u32>() {
            if num > max_id {
                max_id = num;
            }
        }
        search = start;
    }
    let new_id = format!("rId{}", max_id + 1);
    let relationship =
        format!(r#"<Relationship Id="{new_id}" Type="{DOCX_REL_TYPE_HEADER}" Target="{target}"/>"#);

    if let Some(pos) = rels_xml.find("</Relationships>") {
        rels_xml.insert_str(pos, &relationship);
        Ok(new_id)
    } else {
        Err(PdfToolError::new(
            "Invalid document.xml.rels: missing </Relationships>.",
        ))
    }
}

fn ensure_docx_relationship_namespace(document_xml: &mut String) -> Result<()> {
    if document_xml.contains(r#"xmlns:r=""#) {
        return Ok(());
    }

    let Some(start) = document_xml.find("<w:document") else {
        return Err(PdfToolError::new(
            "Invalid word/document.xml: missing <w:document> root.",
        ));
    };
    let Some(end_rel) = document_xml[start..].find('>') else {
        return Err(PdfToolError::new(
            "Invalid word/document.xml: malformed <w:document> root.",
        ));
    };
    let insert_at = start + end_rel;
    let xmlns = format!(r#" xmlns:r="{DOCX_REL_NS}""#);
    document_xml.insert_str(insert_at, &xmlns);
    Ok(())
}

fn upsert_docx_default_header_reference(document_xml: &mut String, rel_id: &str) -> Result<()> {
    let replacement = format!(r#"<w:headerReference w:type="default" r:id="{rel_id}"/>"#);

    let mut replaced_any = false;
    let mut replaced_xml = String::with_capacity(document_xml.len() + 64);
    let mut cursor = 0usize;
    while let Some(start_rel) = document_xml[cursor..].find("<w:headerReference") {
        let start = cursor + start_rel;
        replaced_xml.push_str(&document_xml[cursor..start]);

        let Some(end_rel) = document_xml[start..].find("/>") else {
            break;
        };
        let end = start + end_rel + 2;
        let tag = &document_xml[start..end];
        if tag.contains(r#"w:type="default""#) {
            replaced_xml.push_str(&replacement);
            replaced_any = true;
        } else {
            replaced_xml.push_str(tag);
        }
        cursor = end;
    }
    replaced_xml.push_str(&document_xml[cursor..]);

    if replaced_any {
        *document_xml = replaced_xml;
        return Ok(());
    }

    let mut inserted_any = false;
    let mut inserted_xml = String::with_capacity(document_xml.len() + 128);
    let mut pos = 0usize;
    while let Some(sect_rel) = document_xml[pos..].find("<w:sectPr") {
        let start = pos + sect_rel;
        inserted_xml.push_str(&document_xml[pos..start]);
        let Some(close_rel) = document_xml[start..].find('>') else {
            break;
        };
        let end = start + close_rel + 1;
        inserted_xml.push_str(&document_xml[start..end]);
        inserted_xml.push_str(&replacement);
        inserted_any = true;
        pos = end;
    }
    inserted_xml.push_str(&document_xml[pos..]);

    if inserted_any {
        *document_xml = inserted_xml;
        return Ok(());
    }

    let fallback = format!("<w:sectPr>{replacement}</w:sectPr>");
    if let Some(body_end) = document_xml.find("</w:body>") {
        document_xml.insert_str(body_end, &fallback);
        return Ok(());
    }

    Err(PdfToolError::new(
        "Invalid word/document.xml: failed to locate section properties.",
    ))
}

fn apply_docx_watermark(input_path: &Path, output_path: &Path, watermark_text: &str) -> Result<()> {
    let mut entries = read_zip_entries(input_path)?;

    let mut content_types_xml = read_xml_entry(&entries, "[Content_Types].xml")?;
    let mut rels_xml = read_xml_entry(&entries, "word/_rels/document.xml.rels")?;
    let mut document_xml = read_xml_entry(&entries, "word/document.xml")?;

    let header_target = "header_watermark.xml";
    let header_part_name = "/word/header_watermark.xml";
    let rel_id = ensure_docx_header_relationship(&mut rels_xml, header_target)?;
    ensure_docx_content_type_override(&mut content_types_xml, header_part_name)?;
    ensure_docx_relationship_namespace(&mut document_xml)?;
    upsert_docx_default_header_reference(&mut document_xml, &rel_id)?;

    let header_xml = build_docx_watermark_header_xml(watermark_text);

    entries.insert(
        "[Content_Types].xml".to_string(),
        content_types_xml.into_bytes(),
    );
    entries.insert(
        "word/_rels/document.xml.rels".to_string(),
        rels_xml.into_bytes(),
    );
    entries.insert("word/document.xml".to_string(), document_xml.into_bytes());
    entries.insert(
        "word/header_watermark.xml".to_string(),
        header_xml.into_bytes(),
    );

    write_zip_entries(output_path, &entries)?;
    Ok(())
}

pub(crate) fn apply_watermark(
    input_path: &Path,
    output_path: &Path,
    watermark_text: &str,
) -> Result<()> {
    if has_pdf_extension(input_path) {
        apply_pdf_watermark(input_path, output_path, watermark_text)
    } else if has_docx_extension(input_path) {
        apply_docx_watermark(input_path, output_path, watermark_text)
    } else {
        Err(PdfToolError::new(
            "Watermark currently supports only file types handled by this command.",
        ))
    }
}

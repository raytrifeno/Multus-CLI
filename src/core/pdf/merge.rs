use lopdf::{Bookmark, Document, Object, ObjectId};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::types::{PdfToolError, Result};

pub(crate) fn merge_documents(mut docs: Vec<(Option<String>, Document)>) -> Result<Document> {
    if docs.is_empty() {
        return Err(PdfToolError::new("No input files to merge."));
    }

    let mut max_id = 1;
    let mut pages = BTreeMap::<ObjectId, Object>::new();
    let mut objects = BTreeMap::<ObjectId, Object>::new();
    let mut merged = Document::with_version("1.5");

    for (label, doc) in &mut docs {
        let mut first_page = true;
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        for (_, object_id) in doc.get_pages() {
            if first_page {
                if let Some(title) = label.as_ref() {
                    let bookmark = Bookmark::new(title.clone(), [0.0, 0.0, 0.0], 0, object_id);
                    merged.add_bookmark(bookmark, None);
                }
                first_page = false;
            }
            let object = doc.get_object(object_id).map_err(|e| {
                PdfToolError::new(format!("Failed to read page object while merging: {e}"))
            })?;
            pages.insert(object_id, object.to_owned());
        }
        objects.extend(doc.objects.clone());
    }

    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in &objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                catalog_object = Some((
                    catalog_object.map(|(id, _)| id).unwrap_or(*object_id),
                    object.clone(),
                ));
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref old)) = pages_object
                        && let Ok(old_dictionary) = old.as_dict()
                    {
                        dictionary.extend(old_dictionary);
                    }
                    pages_object = Some((
                        pages_object.map(|(id, _)| id).unwrap_or(*object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                merged.objects.insert(*object_id, object.clone());
            }
        }
    }

    let (catalog_id, catalog_obj) =
        catalog_object.ok_or_else(|| PdfToolError::new("Catalog root not found."))?;
    let (pages_id, pages_obj) =
        pages_object.ok_or_else(|| PdfToolError::new("Pages root not found."))?;

    for (object_id, object) in &pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            merged
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    if let Ok(dictionary) = pages_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", pages.len() as u32);
        dictionary.set(
            "Kids",
            pages
                .keys()
                .map(|id| Object::Reference(*id))
                .collect::<Vec<_>>(),
        );
        merged
            .objects
            .insert(pages_id, Object::Dictionary(dictionary));
    }

    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines");
        merged
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }

    merged.trailer.set("Root", catalog_id);
    merged.max_id = merged.objects.len() as u32;
    merged.renumber_objects();
    merged.adjust_zero_pages();
    if let Some(outline_id) = merged.build_outline()
        && let Ok(catalog) = merged.catalog_mut()
    {
        catalog.set("Outlines", Object::Reference(outline_id));
    }
    merged.compress();
    Ok(merged)
}

pub(crate) fn merge_pdfs(input_paths: &[PathBuf], output_path: &Path) -> Result<()> {
    let mut docs = Vec::new();
    for path in input_paths {
        let doc = Document::load(path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to read file for merging: '{}'. {e}",
                path.display()
            ))
        })?;
        let title = path
            .file_stem()
            .and_then(OsStr::to_str)
            .map(|stem| stem.to_string())
            .unwrap_or_else(|| path.display().to_string());
        docs.push((Some(title), doc));
    }

    let mut merged = merge_documents(docs)?;
    merged
        .save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save merged output: {e}")))?;
    Ok(())
}

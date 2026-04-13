use lopdf::{Dictionary, Document, Object, Stream, dictionary};
use std::path::{Path, PathBuf};

use crate::types::{PdfToolError, Result};

pub(crate) fn images_to_pdf(image_paths: &[PathBuf], output_path: &Path) -> Result<()> {
    if image_paths.is_empty() {
        return Err(PdfToolError::new("No image files provided."));
    }

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_kids: Vec<Object> = Vec::new();

    for image_path in image_paths {
        let image_stream = lopdf::xobject::image(image_path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to read image '{}': {e}",
                image_path.display()
            ))
        })?;

        let width = image_stream
            .dict
            .get(b"Width")
            .and_then(Object::as_i64)
            .map_err(|e| {
                PdfToolError::new(format!(
                    "Failed to detect width for '{}': {e}",
                    image_path.display()
                ))
            })?
            .max(1);
        let height = image_stream
            .dict
            .get(b"Height")
            .and_then(Object::as_i64)
            .map_err(|e| {
                PdfToolError::new(format!(
                    "Failed to detect height for '{}': {e}",
                    image_path.display()
                ))
            })?
            .max(1);

        let content_id = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
        });
        doc.insert_image(
            page_id,
            image_stream,
            (0.0, 0.0),
            (width as f32, height as f32),
        )
        .map_err(|e| {
            PdfToolError::new(format!(
                "Failed to place image on page for '{}': {e}",
                image_path.display()
            ))
        })?;

        page_kids.push(page_id.into());
    }

    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => page_kids,
        "Count" => image_paths.len() as i64,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    doc.renumber_objects();
    doc.compress();
    doc.save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save output file: {e}")))?;
    Ok(())
}

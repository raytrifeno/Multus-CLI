use lopdf::Document;
use std::path::Path;

use crate::core::page::{build_reordered_sequence, parse_page_order};
use crate::core::path::open_pdf;
use crate::core::pdf::merge::merge_documents;
use crate::types::{PdfToolError, Result};

pub(crate) fn reorder_pdf(
    input_path: &Path,
    output_path: &Path,
    page_order_expr: &str,
) -> Result<()> {
    let (pdf_bytes, total_pages) = open_pdf(input_path)?;
    let requested_order = parse_page_order(page_order_expr)?;
    let final_order = build_reordered_sequence(&requested_order, total_pages)?;
    let base_doc = Document::load_mem(&pdf_bytes).map_err(|_| {
        PdfToolError::new(format!(
            "Failed to read document: '{}'",
            input_path.display()
        ))
    })?;

    let mut docs = Vec::with_capacity(final_order.len());
    for page in final_order {
        let mut page_doc = base_doc.clone();

        let existing_pages = page_doc.get_pages();
        let to_delete: Vec<u32> = existing_pages
            .keys()
            .copied()
            .filter(|candidate| *candidate != page)
            .collect();
        if !to_delete.is_empty() {
            page_doc.delete_pages(&to_delete);
        }

        page_doc.renumber_objects();
        page_doc.adjust_zero_pages();
        docs.push((None, page_doc));
    }

    let mut merged = merge_documents(docs)?;
    merged.compress();
    merged
        .save(output_path)
        .map_err(|e| PdfToolError::new(format!("Failed to save reordered output: {e}")))?;
    Ok(())
}

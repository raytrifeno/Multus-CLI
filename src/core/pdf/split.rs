use lopdf::Document;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

use crate::types::{PdfToolError, Result};

pub(crate) fn split_pdf(
    input_path: &Path,
    input_pdf_bytes: &[u8],
    groups: &[Vec<u32>],
    output_dir: &Path,
) -> Result<usize> {
    let stem = input_path
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("output");

    let mut written = 0usize;
    for group in groups {
        if group.is_empty() {
            continue;
        }

        let mut doc = Document::load_mem(input_pdf_bytes).map_err(|_| {
            PdfToolError::new(format!(
                "Failed to read document: '{}'",
                input_path.display()
            ))
        })?;
        let selected: HashSet<u32> = group.iter().copied().collect();

        let existing_pages = doc.get_pages();
        let to_delete: Vec<u32> = existing_pages
            .keys()
            .copied()
            .filter(|page| !selected.contains(page))
            .collect();
        if !to_delete.is_empty() {
            doc.delete_pages(&to_delete);
        }

        doc.renumber_objects();
        doc.adjust_zero_pages();
        doc.compress();

        let label = if group.len() == 1 {
            group[0].to_string()
        } else {
            format!("{}-{}", group[0], group[group.len() - 1])
        };
        let output_path = output_dir.join(format!("{stem}_page_{label}.pdf"));
        doc.save(&output_path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to save split output '{}': {e}",
                output_path.display()
            ))
        })?;
        written += 1;
    }

    Ok(written)
}

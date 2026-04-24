use std::collections::{BTreeSet, HashSet};

use crate::types::{ParsedSelection, PdfToolError, Result};

pub(crate) fn parse_page_selection(selection: &str) -> Result<ParsedSelection> {
    let raw = selection.trim();
    if raw.is_empty() {
        return Err(PdfToolError::new("Page selection is empty."));
    }

    let parts: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(PdfToolError::new("Page selection is invalid."));
    }

    let mut pages = BTreeSet::new();
    let mut groups = Vec::new();

    for part in parts {
        if part.contains('-') {
            let bounds: Vec<&str> = part.split('-').map(str::trim).collect();
            if bounds.len() != 2 || bounds[0].is_empty() || bounds[1].is_empty() {
                return Err(PdfToolError::new(format!("Invalid range: '{part}'")));
            }

            let start = bounds[0]
                .parse::<u32>()
                .map_err(|_| PdfToolError::new(format!("Invalid range numbers: '{part}'")))?;
            let end = bounds[1]
                .parse::<u32>()
                .map_err(|_| PdfToolError::new(format!("Invalid range numbers: '{part}'")))?;

            if start < 1 || end < 1 {
                return Err(PdfToolError::new(format!("Range must be >= 1: '{part}'")));
            }
            if start > end {
                return Err(PdfToolError::new(format!("Range start > end: '{part}'")));
            }

            let mut group = Vec::new();
            for page in start..=end {
                pages.insert(page);
                group.push(page);
            }
            groups.push(group);
        } else {
            let page = part
                .parse::<u32>()
                .map_err(|_| PdfToolError::new(format!("Invalid page number: '{part}'")))?;
            if page < 1 {
                return Err(PdfToolError::new(format!("Page must be >= 1: '{part}'")));
            }
            pages.insert(page);
            groups.push(vec![page]);
        }
    }

    Ok(ParsedSelection {
        pages: pages.into_iter().collect(),
        groups,
    })
}

pub(crate) fn parse_page_order(selection: &str) -> Result<Vec<u32>> {
    let raw = selection.trim();
    if raw.is_empty() {
        return Err(PdfToolError::new("Page order is empty."));
    }

    let parts: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return Err(PdfToolError::new("Page order is invalid."));
    }

    let mut out = Vec::new();
    for part in parts {
        if part.contains('-') {
            let bounds: Vec<&str> = part.split('-').map(str::trim).collect();
            if bounds.len() != 2 || bounds[0].is_empty() || bounds[1].is_empty() {
                return Err(PdfToolError::new(format!("Invalid range: '{part}'")));
            }

            let start = bounds[0]
                .parse::<u32>()
                .map_err(|_| PdfToolError::new(format!("Invalid range numbers: '{part}'")))?;
            let end = bounds[1]
                .parse::<u32>()
                .map_err(|_| PdfToolError::new(format!("Invalid range numbers: '{part}'")))?;
            if start < 1 || end < 1 {
                return Err(PdfToolError::new(format!("Range must be >= 1: '{part}'")));
            }
            if start > end {
                return Err(PdfToolError::new(format!("Range start > end: '{part}'")));
            }
            out.extend(start..=end);
        } else {
            let page = part
                .parse::<u32>()
                .map_err(|_| PdfToolError::new(format!("Invalid page number: '{part}'")))?;
            if page < 1 {
                return Err(PdfToolError::new(format!("Page must be >= 1: '{part}'")));
            }
            out.push(page);
        }
    }

    Ok(out)
}

pub(crate) fn validate_pages(pages: &[u32], total_pages: usize) -> Result<Vec<u32>> {
    if total_pages < 1 {
        return Err(PdfToolError::new("Input has no pages."));
    }

    let max_page = total_pages as u32;
    let out_of_range: Vec<u32> = pages
        .iter()
        .copied()
        .filter(|p| *p < 1 || *p > max_page)
        .collect();
    if !out_of_range.is_empty() {
        let joined = out_of_range
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PdfToolError::new(format!(
            "Pages out of range (1-{total_pages}): {joined}"
        )));
    }
    Ok(pages.to_vec())
}

pub(crate) fn build_reordered_sequence(requested: &[u32], total_pages: usize) -> Result<Vec<u32>> {
    validate_pages(requested, total_pages)?;

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for &page in requested {
        if !seen.insert(page) {
            return Err(PdfToolError::new(format!(
                "Duplicate page in order: '{page}'"
            )));
        }
        out.push(page);
    }

    for page in 1..=(total_pages as u32) {
        if seen.insert(page) {
            out.push(page);
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{build_reordered_sequence, parse_page_order};

    #[test]
    fn reorder_rejects_duplicate_requested_pages() {
        let order = parse_page_order("3,1,3").unwrap();
        let result = build_reordered_sequence(&order, 4);

        assert!(result.is_err());
    }
}

use std::env;

use crate::cli::SplitArgs;
use crate::core::page::{parse_page_selection, validate_pages};
use crate::types::{PdfToolError, Result};

pub(crate) fn handle_split(args: SplitArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };

    let input_path = crate::resolve_input_path(&input_value)?;
    let (pdf_bytes, total_pages) =
        crate::run_with_spinner("Verifying file...", || crate::open_pdf(&input_path))?;

    let pages_value = if let Some(pages) = args.pages {
        pages
    } else {
        crate::print_step("SELECT PAGES");
        crate::prompt_non_empty(r#"Enter page range (example "1-5,8,10-12"): "#)?
    };

    let selection = parse_page_selection(&pages_value)?;
    validate_pages(&selection.pages, total_pages)?;

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        let value =
            crate::prompt_optional("Save to which directory? (empty = current directory): ")?;
        if value.is_empty() {
            env::current_dir()
                .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
                .to_string_lossy()
                .to_string()
        } else {
            value
        }
    };

    let output_dir = crate::ensure_output_dir(Some(&output_value))?;
    let count = crate::split_pdf(&input_path, &pdf_bytes, &selection.groups, &output_dir)?;
    println!("Saved {count} file(s) to: {}", output_dir.display());
    Ok(0)
}

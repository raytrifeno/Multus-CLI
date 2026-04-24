use crate::cli::WatermarkArgs;
use crate::commands::common::{default_output_name, ensure_file_exists};
use crate::types::{PdfToolError, Result};

pub(crate) fn handle_watermark(args: WatermarkArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter a supported file path: ")?
    };
    let input_path = crate::resolve_input_path(&input_value)?;
    ensure_file_exists(&input_path)?;
    if !crate::has_pdf_extension(&input_path) && !crate::has_docx_extension(&input_path) {
        return Err(PdfToolError::new(
            "Watermark currently supports only file types handled by this command.",
        ));
    }

    let watermark_text = if let Some(text) = args.text {
        let cleaned = text.trim().to_string();
        if cleaned.is_empty() {
            "CONFIDENTIAL".to_string()
        } else {
            cleaned
        }
    } else {
        crate::print_step("WATERMARK TEXT");
        let value = crate::prompt_optional("Enter watermark text (empty = CONFIDENTIAL): ")?;
        if value.is_empty() {
            "CONFIDENTIAL".to_string()
        } else {
            value
        }
    };

    let ext = if crate::has_docx_extension(&input_path) {
        "docx"
    } else {
        "pdf"
    };
    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional(&format!(
            "Save as? (empty = {}): ",
            default_output_name(&input_path, "watermarked", ext)
        ))?
    };

    let default_name = default_output_name(&input_path, "watermarked", ext);
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;
    crate::ensure_output_is_not_input(&output_path, std::slice::from_ref(&input_path))?;

    crate::run_with_spinner("Applying watermark...", || {
        crate::apply_watermark(&input_path, &output_path, &watermark_text)
    })?;

    println!("Watermark complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

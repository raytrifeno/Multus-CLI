use crate::cli::ReorderArgs;
use crate::commands::common::{default_output_name, ensure_pdf_input};
use crate::types::Result;

pub(crate) fn handle_reorder(args: ReorderArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };
    let input_path = crate::resolve_input_path(&input_value)?;
    ensure_pdf_input(&input_path)?;

    let order_value = if let Some(pages) = args.pages {
        pages
    } else {
        crate::print_step("ORDER");
        crate::prompt_non_empty(r#"Enter new page order (example "10,1-9"): "#)?
    };

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = default_output_name(&input_path, "reordered", "pdf");
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;
    crate::ensure_output_is_not_input(&output_path, std::slice::from_ref(&input_path))?;

    crate::run_with_spinner("Reordering pages...", || {
        crate::reorder_pdf(&input_path, &output_path, &order_value)
    })?;
    println!("Reorder complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

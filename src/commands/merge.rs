use crate::cli::MergeArgs;
use crate::commands::common::{
    ensure_pdf_input, prompt_path_list, resolve_input_paths, sort_paths_naturally,
};
use crate::types::Result;

pub(crate) fn handle_merge(args: MergeArgs) -> Result<i32> {
    let input_values = if !args.inputs.is_empty() {
        args.inputs
    } else {
        prompt_path_list(
            "INPUT FILES",
            "Enter file paths (you can drag many files at once; separators: space/comma/semicolon): ",
        )?
    };

    let mut input_paths = resolve_input_paths(&input_values)?;
    sort_paths_naturally(&mut input_paths);
    for path in &input_paths {
        ensure_pdf_input(path)?;
    }

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_non_empty("Save as? (example: merged-file): ")?
    };

    let default_name = "merged.pdf".to_string();
    let output_path =
        crate::build_output_file_path(&input_paths[0], Some(&output_value), &default_name)?;
    crate::ensure_output_is_not_input(&output_path, &input_paths)?;

    crate::run_with_spinner("Merging files...", || {
        crate::merge_pdfs(&input_paths, &output_path)
    })?;
    println!("Merge complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

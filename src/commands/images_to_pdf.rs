use crate::cli::ImagesToPdfArgs;
use crate::commands::common::{
    default_output_name, ensure_non_empty_inputs, ensure_supported_image_input, prompt_path_list,
    resolve_input_paths,
};
use crate::types::Result;

pub(crate) fn handle_images_to_pdf(args: ImagesToPdfArgs) -> Result<i32> {
    let input_values = if !args.inputs.is_empty() {
        args.inputs
    } else {
        prompt_path_list(
            "INPUT IMAGES",
            "Enter image file paths (separate with spaces or commas): ",
        )?
    };

    let input_paths = resolve_input_paths(&input_values)?;
    ensure_non_empty_inputs(&input_paths, "No image files were provided.")?;
    for path in &input_paths {
        ensure_supported_image_input(path)?;
    }

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = default_output_name(&input_paths[0], "images", "pdf");
    let output_path =
        crate::build_output_file_path(&input_paths[0], Some(&output_value), &default_name)?;
    crate::ensure_output_is_not_input(&output_path, &input_paths)?;

    crate::run_with_spinner("Building output from images...", || {
        crate::images_to_pdf(&input_paths, &output_path)
    })?;

    println!("Conversion complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

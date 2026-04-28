use std::env;
use std::path::PathBuf;

use crate::cli::ConvertImageArgs;
use crate::commands::common::{
    default_output_name, ensure_non_empty_inputs, ensure_supported_image_input, prompt_path_list,
    resolve_input_paths,
};
use crate::types::{PdfToolError, Result};

fn normalize_target_image_format(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Ok("jpg"),
        "png" => Ok("png"),
        _ => Err(PdfToolError::new(
            "Invalid image format. Supported values: jpg, png.",
        )),
    }
}

pub(crate) fn handle_convert_image(args: ConvertImageArgs) -> Result<i32> {
    let input_values = if !args.inputs.is_empty() {
        args.inputs
    } else {
        prompt_path_list(
            "INPUT IMAGES",
            "Enter image file paths (you can drag many files at once; separators: space/comma/semicolon): ",
        )?
    };

    let input_paths = resolve_input_paths(&input_values)?;
    ensure_non_empty_inputs(&input_paths, "No image files were provided.")?;
    for path in &input_paths {
        ensure_supported_image_input(path)?;
    }

    let format_value = if let Some(format) = args.format {
        format
    } else {
        crate::print_step("TARGET FORMAT");
        crate::prompt_non_empty("Convert to which format? (jpg/png): ")?
    };
    let target_ext = normalize_target_image_format(&format_value)?;

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output path): ")?
    };

    if input_paths.len() == 1 {
        let input_path = &input_paths[0];
        let default_name = default_output_name(input_path, "converted", target_ext);
        let output_path =
            crate::build_output_file_path(input_path, Some(&output_value), &default_name)?;
        crate::ensure_output_is_not_input(&output_path, std::slice::from_ref(input_path))?;

        crate::run_with_spinner("Converting image...", || {
            crate::convert_image_format(input_path, &output_path, target_ext)
        })?;
        println!("Conversion complete!");
        println!("Saved to: {}", output_path.display());
        return Ok(0);
    }

    if !output_value.trim().is_empty() && PathBuf::from(output_value.trim()).extension().is_some() {
        return Err(PdfToolError::new(
            "For multiple images, output must be a directory path.",
        ));
    }

    let output_dir = if output_value.trim().is_empty() {
        env::current_dir()
            .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
    } else {
        crate::ensure_output_dir(Some(&output_value))?
    };

    let converted_count = crate::run_with_spinner("Converting images...", || {
        for input_path in &input_paths {
            let output_path =
                output_dir.join(default_output_name(input_path, "converted", target_ext));
            crate::ensure_output_is_not_input(&output_path, std::slice::from_ref(input_path))?;
            crate::convert_image_format(input_path, &output_path, target_ext)?;
        }
        Ok(input_paths.len())
    })?;

    println!("Conversion complete!");
    println!("Converted {converted_count} file(s) to .{target_ext}");
    println!("Saved to: {}", output_dir.display());
    Ok(0)
}

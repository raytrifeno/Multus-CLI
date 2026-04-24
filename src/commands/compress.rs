use crate::cli::CompressArgs;
use crate::commands::common::{default_output_name, ensure_pdf_input};
use crate::types::Result;

pub(crate) fn handle_compress(args: CompressArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };

    let input_path = crate::resolve_input_path(&input_value)?;
    ensure_pdf_input(&input_path)?;

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = default_output_name(&input_path, "compressed", "pdf");
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;
    crate::ensure_output_is_not_input(&output_path, std::slice::from_ref(&input_path))?;

    let stats = crate::run_with_spinner("Compressing file...", || {
        crate::compress_pdf(&input_path, &output_path, args.level)
    })?;

    let original_size = stats.original_size;
    let compressed_size = stats.output_size;
    let reduction = if original_size == 0 {
        0.0
    } else {
        (1.0 - (compressed_size as f64 / original_size as f64)) * 100.0
    };

    println!("Compression complete!");
    if stats.fallback_to_original {
        println!(
            "This file is already optimized: compressed output was larger, so the original size was kept."
        );
    }
    println!("Level:           {}", stats.level);
    println!("Images optimized: {}", stats.images_optimized);
    println!(
        "Original size:   {:.2} MB",
        original_size as f64 / 1024.0 / 1024.0
    );
    println!(
        "Compressed size: {:.2} MB",
        compressed_size as f64 / 1024.0 / 1024.0
    );
    println!("Reduction:       {reduction:.2}%");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

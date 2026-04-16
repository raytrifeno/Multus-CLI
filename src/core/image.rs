use image::{DynamicImage, ImageFormat};
use std::fs;
use std::path::Path;

use crate::types::{PdfToolError, Result};

pub(crate) fn convert_image_format(
    input_path: &Path,
    output_path: &Path,
    target_format: &str,
) -> Result<()> {
    if input_path == output_path {
        return Err(PdfToolError::new(
            "Output path must be different from input path.",
        ));
    }

    let img = image::open(input_path).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to open image '{}': {e}",
            input_path.display()
        ))
    })?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to create output directory '{}': {e}",
                parent.display()
            ))
        })?;
    }

    match target_format {
        "png" => img
            .save_with_format(output_path, ImageFormat::Png)
            .map_err(|e| PdfToolError::new(format!("Failed to save PNG output: {e}")))?,
        "jpg" => {
            let rgb = img.to_rgb8();
            DynamicImage::ImageRgb8(rgb)
                .save_with_format(output_path, ImageFormat::Jpeg)
                .map_err(|e| PdfToolError::new(format!("Failed to save JPG output: {e}")))?;
        }
        _ => {
            return Err(PdfToolError::new(format!(
                "Unsupported target image format: '{target_format}'."
            )));
        }
    }

    Ok(())
}

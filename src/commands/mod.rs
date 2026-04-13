use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

use crate::cli::{
    CompressArgs, EncryptArgs, ImagesToPdfArgs, MergeArgs, ReorderArgs, SplitArgs, UpdateArgs,
    WatermarkArgs,
};
use crate::core::page::{parse_page_selection, validate_pages};
use crate::types::{PdfToolError, Result};
use crate::update::{
    UPDATE_REPO_REF, UPDATE_REPO_URL, VersionState, check_version_state, update_multus,
};

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

pub(crate) fn handle_compress(args: CompressArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };

    let input_path = crate::resolve_input_path(&input_value)?;
    if !input_path.exists() {
        return Err(PdfToolError::new(format!(
            "File not found: '{}'",
            input_path.display()
        )));
    }
    if !crate::has_pdf_extension(&input_path) {
        return Err(PdfToolError::new(format!(
            "Input format is not supported for this command: '{}'",
            input_path.display()
        )));
    }

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = format!(
        "{}_compressed.pdf",
        input_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("output")
    );
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;

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

pub(crate) fn handle_merge(args: MergeArgs) -> Result<i32> {
    let input_values = if !args.inputs.is_empty() {
        args.inputs
    } else {
        crate::print_step("INPUT FILES");
        let raw = crate::prompt_non_empty("Enter file paths (separate with spaces or commas): ")?;
        if raw.contains(',') {
            raw.split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        } else {
            raw.split_whitespace()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        }
    };

    let input_paths: Vec<PathBuf> = input_values
        .iter()
        .map(|value| crate::resolve_input_path(value))
        .collect::<Result<Vec<_>>>()?;
    for path in &input_paths {
        if !path.exists() {
            return Err(PdfToolError::new(format!(
                "File not found: '{}'",
                path.display()
            )));
        }
        if !crate::has_pdf_extension(path) {
            return Err(PdfToolError::new(format!(
                "Input format is not supported for this command: '{}'",
                path.display()
            )));
        }
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

    crate::run_with_spinner("Merging files...", || {
        crate::merge_pdfs(&input_paths, &output_path)
    })?;
    println!("Merge complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

pub(crate) fn handle_encrypt(args: EncryptArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };
    let input_path = crate::resolve_input_path(&input_value)?;
    if !input_path.exists() {
        return Err(PdfToolError::new(format!(
            "File not found: '{}'",
            input_path.display()
        )));
    }
    if !crate::has_pdf_extension(&input_path) {
        return Err(PdfToolError::new(format!(
            "Input format is not supported for this command: '{}'",
            input_path.display()
        )));
    }

    let prompted_password = args.password.is_none();
    let password = if let Some(pass) = args.password.as_ref() {
        pass.trim().to_string()
    } else {
        crate::print_step("PASSWORD");
        crate::prompt_non_empty("Enter password: ")?
    };
    if password.is_empty() {
        return Err(PdfToolError::new("Password cannot be empty."));
    }

    let owner_password = if let Some(owner) = args.owner_password {
        let cleaned = owner.trim().to_string();
        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    } else if prompted_password {
        crate::print_step("OWNER PASSWORD");
        let value =
            crate::prompt_optional("Enter owner password (empty = same as user password): ")?;
        if value.is_empty() { None } else { Some(value) }
    } else {
        None
    };

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = format!(
        "{}_encrypted.pdf",
        input_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("output")
    );
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;

    crate::run_with_spinner("Encrypting file...", || {
        crate::encrypt_pdf(
            &input_path,
            &output_path,
            &password,
            owner_password.as_deref(),
        )
    })?;
    println!("Encryption complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

pub(crate) fn handle_images_to_pdf(args: ImagesToPdfArgs) -> Result<i32> {
    let input_values = if !args.inputs.is_empty() {
        args.inputs
    } else {
        crate::print_step("INPUT IMAGES");
        let raw =
            crate::prompt_non_empty("Enter image file paths (separate with spaces or commas): ")?;
        if raw.contains(',') {
            raw.split(',')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        } else {
            raw.split_whitespace()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        }
    };

    let input_paths: Vec<PathBuf> = input_values
        .iter()
        .map(|value| crate::resolve_input_path(value))
        .collect::<Result<Vec<_>>>()?;
    if input_paths.is_empty() {
        return Err(PdfToolError::new("No image files were provided."));
    }
    for path in &input_paths {
        if !path.exists() {
            return Err(PdfToolError::new(format!(
                "File not found: '{}'",
                path.display()
            )));
        }
        if !crate::has_supported_image_extension(path) {
            return Err(PdfToolError::new(format!(
                "Unsupported image format: '{}'. Supported: png, jpg, jpeg, bmp, gif, tif, tiff",
                path.display()
            )));
        }
    }

    let output_value = if let Some(output) = args.output {
        output
    } else {
        crate::print_step("OUTPUT");
        crate::prompt_optional("Save as? (empty = auto output name): ")?
    };

    let default_name = format!(
        "{}_images.pdf",
        input_paths[0]
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("output")
    );
    let output_path =
        crate::build_output_file_path(&input_paths[0], Some(&output_value), &default_name)?;

    crate::run_with_spinner("Building output from images...", || {
        crate::images_to_pdf(&input_paths, &output_path)
    })?;

    println!("Conversion complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

pub(crate) fn handle_watermark(args: WatermarkArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter a supported file path: ")?
    };
    let input_path = crate::resolve_input_path(&input_value)?;
    if !input_path.exists() {
        return Err(PdfToolError::new(format!(
            "File not found: '{}'",
            input_path.display()
        )));
    }
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
            "Save as? (empty = {}_watermarked.{ext}): ",
            input_path
                .file_stem()
                .and_then(OsStr::to_str)
                .unwrap_or("output")
        ))?
    };

    let default_name = format!(
        "{}_watermarked.{ext}",
        input_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("output")
    );
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;

    crate::run_with_spinner("Applying watermark...", || {
        crate::apply_watermark(&input_path, &output_path, &watermark_text)
    })?;

    println!("Watermark complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

pub(crate) fn handle_reorder(args: ReorderArgs) -> Result<i32> {
    let input_value = if let Some(input) = args.input {
        input
    } else {
        crate::print_step("INPUT FILE");
        crate::prompt_non_empty("Enter the file path: ")?
    };
    let input_path = crate::resolve_input_path(&input_value)?;
    if !input_path.exists() {
        return Err(PdfToolError::new(format!(
            "File not found: '{}'",
            input_path.display()
        )));
    }
    if !crate::has_pdf_extension(&input_path) {
        return Err(PdfToolError::new(format!(
            "Input format is not supported for this command: '{}'",
            input_path.display()
        )));
    }

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

    let default_name = format!(
        "{}_reordered.pdf",
        input_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("output")
    );
    let output_path =
        crate::build_output_file_path(&input_path, Some(&output_value), &default_name)?;

    crate::run_with_spinner("Reordering pages...", || {
        crate::reorder_pdf(&input_path, &output_path, &order_value)
    })?;
    println!("Reorder complete!");
    println!("Saved to: {}", output_path.display());
    Ok(0)
}

pub(crate) fn handle_update(args: UpdateArgs) -> Result<i32> {
    let repo = args.repo.unwrap_or_else(|| UPDATE_REPO_URL.to_string());
    let branch = args.branch.unwrap_or_else(|| UPDATE_REPO_REF.to_string());

    match check_version_state(&repo, &branch) {
        VersionState::UpToDate { current } => {
            println!("Sudah versi terbaru (v{current}).");
            return Ok(0);
        }
        VersionState::UpdateAvailable { current, latest } => {
            println!("Update tersedia: v{current} -> v{latest}");
        }
        VersionState::Unknown { current } => {
            println!("Version current: v{current}");
            println!("Tidak bisa memverifikasi versi remote, mencoba update langsung...");
        }
    }

    println!("Updating from: {repo} (branch: {branch})");
    crate::run_with_spinner("Updating multus...", || update_multus(&repo, &branch))?;

    println!("Update complete!");
    println!("Run 'multus --version' to verify current version.");
    Ok(0)
}

use directories::BaseDirs;
use lopdf::Document;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::types::{PdfToolError, Result};

const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff"];

pub(crate) fn strip_wrapping_quotes(value: &str) -> String {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'') && first == last {
            return trimmed[1..trimmed.len() - 1].trim().to_string();
        }
    }
    trimmed.to_string()
}

fn user_home_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

fn expand_user(path_text: &str) -> PathBuf {
    if path_text == "~"
        && let Some(home) = user_home_dir()
    {
        return home;
    }

    if let Some(rest) = path_text.strip_prefix("~/")
        && let Some(home) = user_home_dir()
    {
        return home.join(rest);
    }

    if let Some(rest) = path_text.strip_prefix("~\\")
        && let Some(home) = user_home_dir()
    {
        return home.join(rest);
    }

    PathBuf::from(path_text)
}

pub(crate) fn resolve_input_path(input_arg: &str) -> Result<PathBuf> {
    let cleaned = strip_wrapping_quotes(input_arg);
    let expanded = expand_user(&cleaned);
    if expanded.is_absolute() {
        Ok(expanded)
    } else {
        let cwd = env::current_dir()
            .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?;
        Ok(cwd.join(expanded))
    }
}

pub(crate) fn ensure_output_dir(output_arg: Option<&str>) -> Result<PathBuf> {
    let cleaned = output_arg
        .map(strip_wrapping_quotes)
        .unwrap_or_default()
        .trim()
        .to_string();

    let output_dir = if cleaned.is_empty() {
        env::current_dir()
            .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
    } else {
        let base = expand_user(&cleaned);
        if base.is_absolute() {
            base
        } else {
            env::current_dir()
                .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
                .join(base)
        }
    };

    fs::create_dir_all(&output_dir).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to create output directory '{}': {e}",
            output_dir.display()
        ))
    })?;
    Ok(output_dir)
}

pub(crate) fn has_pdf_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

pub(crate) fn has_docx_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| ext.eq_ignore_ascii_case("docx"))
        .unwrap_or(false)
}

pub(crate) fn has_supported_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            SUPPORTED_IMAGE_EXTENSIONS
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false)
}

pub(crate) fn open_pdf(input_path: &Path) -> Result<(Vec<u8>, usize)> {
    if !has_pdf_extension(input_path) {
        return Err(PdfToolError::new(format!(
            "Input format is not supported for this command: '{}'",
            input_path.display()
        )));
    }

    let bytes = fs::read(input_path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            PdfToolError::new(format!("File not found: '{}'", input_path.display()))
        } else if e.kind() == io::ErrorKind::IsADirectory {
            PdfToolError::new(format!(
                "Input path is a directory: '{}'",
                input_path.display()
            ))
        } else {
            PdfToolError::new(format!(
                "Failed to open file: '{}': {e}",
                input_path.display()
            ))
        }
    })?;

    let document = Document::load_mem(&bytes).map_err(|_| {
        PdfToolError::new(format!(
            "Failed to read document: '{}'",
            input_path.display()
        ))
    })?;
    let total_pages = document.get_pages().len();
    if total_pages < 1 {
        return Err(PdfToolError::new("Input has no pages."));
    }
    Ok((bytes, total_pages))
}

fn absolute_path_for_compare(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path).map_err(|e| {
            PdfToolError::new(format!("Failed to resolve path '{}': {e}", path.display()))
        });
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
            .join(path)
    };

    if let (Some(parent), Some(file_name)) = (absolute.parent(), absolute.file_name())
        && parent.exists()
    {
        let parent = fs::canonicalize(parent).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to resolve output directory '{}': {e}",
                parent.display()
            ))
        })?;
        return Ok(parent.join(file_name));
    }

    Ok(absolute)
}

fn paths_match(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

pub(crate) fn ensure_output_is_not_input(
    output_path: &Path,
    input_paths: &[PathBuf],
) -> Result<()> {
    let output = absolute_path_for_compare(output_path)?;
    for input_path in input_paths {
        let input = absolute_path_for_compare(input_path)?;
        if paths_match(&output, &input) {
            return Err(PdfToolError::new(format!(
                "Output path must be different from input path: '{}'",
                output_path.display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn build_output_file_path(
    input_path: &Path,
    output_arg: Option<&str>,
    default_filename: &str,
) -> Result<PathBuf> {
    let raw_output = output_arg
        .map(strip_wrapping_quotes)
        .unwrap_or_default()
        .trim()
        .to_string();

    let mut output_path = if raw_output.is_empty() {
        input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(default_filename)
    } else {
        let given = expand_user(&raw_output);
        if given.is_absolute() {
            given
        } else {
            env::current_dir()
                .map_err(|e| PdfToolError::new(format!("Failed to read current directory: {e}")))?
                .join(given)
        }
    };

    if output_path.is_dir() {
        output_path = output_path.join(default_filename);
    } else if output_path.extension().is_none() {
        fs::create_dir_all(&output_path).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to create output directory '{}': {e}",
                output_path.display()
            ))
        })?;
        output_path = output_path.join(default_filename);
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to create output directory '{}': {e}",
                parent.display()
            ))
        })?;
    }

    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::{ensure_output_is_not_input, has_pdf_extension, strip_wrapping_quotes};
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn strip_wrapping_quotes_handles_common_inputs() {
        assert_eq!(strip_wrapping_quotes("\"hello\""), "hello");
        assert_eq!(strip_wrapping_quotes("'world'"), "world");
        assert_eq!(strip_wrapping_quotes("  no-quotes  "), "no-quotes");
        assert_eq!(strip_wrapping_quotes("\" spaced \""), "spaced");
    }

    #[test]
    fn has_pdf_extension_is_case_insensitive() {
        assert!(has_pdf_extension(Path::new("file.pdf")));
        assert!(has_pdf_extension(Path::new("file.PDF")));
        assert!(!has_pdf_extension(Path::new("file.docx")));
        assert!(!has_pdf_extension(Path::new("file")));
    }

    #[test]
    fn output_path_cannot_point_to_input_file() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("multus-path-test-{nonce}"));
        fs::create_dir_all(&dir).unwrap();
        let input = dir.join("input.pdf");
        fs::write(&input, b"placeholder").unwrap();

        let result = ensure_output_is_not_input(&input, std::slice::from_ref(&input));
        let _ = fs::remove_file(&input);
        let _ = fs::remove_dir(&dir);

        assert!(result.is_err());
    }
}

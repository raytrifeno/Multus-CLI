use std::path::{Path, PathBuf};

use crate::types::{PdfToolError, Result};

pub(crate) fn parse_path_list(raw: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if quote.is_none() => match chars.peek().copied() {
                Some(next)
                    if next.is_whitespace() || matches!(next, ',' | ';' | '"' | '\'' | '\\') =>
                {
                    current.push(next);
                    let _ = chars.next();
                }
                _ => current.push('\\'),
            },
            '"' | '\'' if quote == Some(ch) => quote = None,
            '"' | '\'' if quote.is_none() => quote = Some(ch),
            ',' | ';' if quote.is_none() => push_current_path(&mut items, &mut current),
            ch if ch.is_whitespace() && quote.is_none() => {
                push_current_path(&mut items, &mut current);
            }
            _ => current.push(ch),
        }
    }

    push_current_path(&mut items, &mut current);
    items
}

fn push_current_path(items: &mut Vec<String>, current: &mut String) {
    let value = current.trim();
    if !value.is_empty() {
        items.push(value.to_string());
    }
    current.clear();
}

pub(crate) fn prompt_path_list(title: &str, prompt: &str) -> Result<Vec<String>> {
    crate::print_step(title);
    let raw = crate::prompt_non_empty(prompt)?;
    Ok(parse_path_list(&raw))
}

pub(crate) fn resolve_input_paths(input_values: &[String]) -> Result<Vec<PathBuf>> {
    input_values
        .iter()
        .map(|value| crate::resolve_input_path(value))
        .collect::<Result<Vec<_>>>()
}

pub(crate) fn ensure_pdf_input(path: &Path) -> Result<()> {
    ensure_file_exists(path)?;
    if !crate::has_pdf_extension(path) {
        return Err(PdfToolError::new(format!(
            "Input format is not supported for this command: '{}'",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn ensure_supported_image_input(path: &Path) -> Result<()> {
    ensure_file_exists(path)?;
    if !crate::has_supported_image_extension(path) {
        return Err(PdfToolError::new(format!(
            "Unsupported image format: '{}'. Supported: png, jpg, jpeg, bmp, gif, tif, tiff",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn ensure_file_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(PdfToolError::new(format!(
            "File not found: '{}'",
            path.display()
        )));
    }
    Ok(())
}

pub(crate) fn default_output_name(input_path: &Path, suffix: &str, extension: &str) -> String {
    let stem = input_path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("output");
    format!("{stem}_{suffix}.{extension}")
}

pub(crate) fn ensure_non_empty_inputs(paths: &[PathBuf], message: &str) -> Result<()> {
    if paths.is_empty() {
        return Err(PdfToolError::new(message));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_path_list;

    #[test]
    fn parse_path_list_keeps_quoted_paths_with_spaces() {
        let items = parse_path_list(r#""C:\docs\a file.pdf" "C:\docs\b file.pdf""#);

        assert_eq!(
            items,
            vec![r#"C:\docs\a file.pdf"#, r#"C:\docs\b file.pdf"#]
        );
    }

    #[test]
    fn parse_path_list_supports_commas_outside_quotes() {
        let items = parse_path_list(r#""C:\docs\a, one.pdf", C:\docs\b.pdf"#);

        assert_eq!(items, vec![r#"C:\docs\a, one.pdf"#, r#"C:\docs\b.pdf"#]);
    }

    #[test]
    fn parse_path_list_supports_shell_escaped_spaces() {
        let items = parse_path_list("/home/me/a\\ file.pdf /home/me/b\\ file.pdf");

        assert_eq!(items, vec!["/home/me/a file.pdf", "/home/me/b file.pdf"]);
    }

    #[test]
    fn parse_path_list_supports_semicolon_delimiter() {
        let items = parse_path_list(r#""C:\docs\a file.pdf";"C:\docs\b file.pdf""#);

        assert_eq!(
            items,
            vec![r#"C:\docs\a file.pdf"#, r#"C:\docs\b file.pdf"#]
        );
    }

    #[test]
    fn parse_path_list_supports_escaped_delimiters() {
        let items = parse_path_list(r#"C:\docs\a\,one.pdf C:\docs\b\;two.pdf"#);

        assert_eq!(items, vec![r#"C:\docs\a,one.pdf"#, r#"C:\docs\b;two.pdf"#]);
    }
}

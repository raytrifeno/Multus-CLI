use std::cmp::Ordering;
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

pub(crate) fn sort_paths_naturally(paths: &mut [PathBuf]) {
    paths.sort_by(|left, right| compare_paths_naturally(left, right));
}

fn compare_paths_naturally(left: &Path, right: &Path) -> Ordering {
    let left_parent = comparable_parent(left);
    let right_parent = comparable_parent(right);
    let parent_order = natural_compare(&left_parent, &right_parent);
    if parent_order != Ordering::Equal {
        return parent_order;
    }

    let left_name = comparable_file_name(left);
    let right_name = comparable_file_name(right);
    natural_compare(&left_name, &right_name)
}

fn comparable_parent(path: &Path) -> String {
    path.parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn comparable_file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn natural_compare(left: &str, right: &str) -> Ordering {
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();

    loop {
        match (left_chars.peek().copied(), right_chars.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(left_ch), Some(right_ch))
                if left_ch.is_ascii_digit() && right_ch.is_ascii_digit() =>
            {
                let left_number = take_number(&mut left_chars);
                let right_number = take_number(&mut right_chars);
                let number_order = compare_number_text(&left_number, &right_number);
                if number_order != Ordering::Equal {
                    return number_order;
                }
            }
            (Some(left_ch), Some(right_ch)) => {
                let _ = left_chars.next();
                let _ = right_chars.next();
                let char_order = left_ch
                    .to_ascii_lowercase()
                    .cmp(&right_ch.to_ascii_lowercase());
                if char_order != Ordering::Equal {
                    return char_order;
                }
            }
        }
    }
}

fn take_number<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut number = String::new();
    while let Some(ch) = chars.peek().copied() {
        if ch.is_ascii_digit() {
            number.push(ch);
            let _ = chars.next();
        } else {
            break;
        }
    }
    number
}

fn compare_number_text(left: &str, right: &str) -> Ordering {
    let left_trimmed = left.trim_start_matches('0');
    let right_trimmed = right.trim_start_matches('0');
    let left_value = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_value = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };

    match left_value.len().cmp(&right_value.len()) {
        Ordering::Equal => match left_value.cmp(right_value) {
            Ordering::Equal => left.len().cmp(&right.len()),
            other => other,
        },
        other => other,
    }
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
    use super::{natural_compare, parse_path_list, sort_paths_naturally};
    use std::cmp::Ordering;
    use std::path::PathBuf;

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

    #[test]
    fn natural_compare_orders_numbers_by_value() {
        assert_eq!(natural_compare("file2.pdf", "file10.pdf"), Ordering::Less);
    }

    #[test]
    fn sort_paths_naturally_orders_by_parent_then_file_name() {
        let mut paths = vec![
            PathBuf::from(r"C:\docs\10.pdf"),
            PathBuf::from(r"C:\docs\2.pdf"),
            PathBuf::from(r"C:\docs\A.pdf"),
            PathBuf::from(r"C:\docs\E.pdf"),
            PathBuf::from(r"C:\docs\C.pdf"),
        ];

        sort_paths_naturally(&mut paths);

        assert_eq!(
            paths,
            vec![
                PathBuf::from(r"C:\docs\2.pdf"),
                PathBuf::from(r"C:\docs\10.pdf"),
                PathBuf::from(r"C:\docs\A.pdf"),
                PathBuf::from(r"C:\docs\C.pdf"),
                PathBuf::from(r"C:\docs\E.pdf"),
            ]
        );
    }
}

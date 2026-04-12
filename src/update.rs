use std::cmp::Ordering;
use std::process::Command;

use crate::types::{PdfToolError, Result};

pub const UPDATE_REPO_URL: &str = "https://github.com/raytrifeno/scraks.git";
pub const UPDATE_REPO_REF: &str = "main";

#[derive(Debug, Clone)]
pub(crate) enum VersionState {
    UpToDate { current: String },
    UpdateAvailable { current: String, latest: String },
    Unknown { current: String },
}

pub(crate) fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn parse_github_owner_repo(repo: &str) -> Option<(String, String)> {
    let cleaned = repo.trim().trim_end_matches('/');
    let cleaned = cleaned.strip_suffix(".git").unwrap_or(cleaned);

    let path = if let Some(rest) = cleaned.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = cleaned.strip_prefix("http://github.com/") {
        rest
    } else if let Some(rest) = cleaned.strip_prefix("git@github.com:") {
        rest
    } else {
        return None;
    };

    let mut parts = path.split('/');
    let owner = parts.next()?.trim();
    let name = parts.next()?.trim();
    if owner.is_empty() || name.is_empty() {
        return None;
    }

    Some((owner.to_string(), name.to_string()))
}

fn extract_version_from_cargo_toml(cargo_toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }

        if in_package && trimmed.starts_with("version") {
            let mut parts = trimmed.splitn(2, '=');
            let _ = parts.next()?;
            let raw = parts.next()?.trim();
            return Some(raw.trim_matches('"').to_string());
        }
    }

    None
}

fn parse_version_segments(version: &str) -> Vec<u64> {
    let normalized = version.trim().trim_start_matches('v');
    let core = normalized
        .split(['-', '+'])
        .next()
        .unwrap_or(normalized);

    core.split('.')
        .map(|segment| {
            let digits = segment
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<u64>().unwrap_or(0)
        })
        .collect()
}

fn compare_version_strings(current: &str, latest: &str) -> Ordering {
    let a = parse_version_segments(current);
    let b = parse_version_segments(latest);

    if a.is_empty() || b.is_empty() {
        return current.cmp(latest);
    }

    let len = a.len().max(b.len());
    for index in 0..len {
        let left = *a.get(index).unwrap_or(&0);
        let right = *b.get(index).unwrap_or(&0);
        match left.cmp(&right) {
            Ordering::Equal => {}
            non_equal => return non_equal,
        }
    }

    Ordering::Equal
}

pub(crate) fn fetch_remote_version(repo: &str, branch: &str) -> Result<String> {
    let (owner, name) = parse_github_owner_repo(repo).ok_or_else(|| {
        PdfToolError::new(format!(
            "Unsupported repository format for version check: '{repo}'"
        ))
    })?;

    let url = format!(
        "https://raw.githubusercontent.com/{owner}/{name}/{branch}/Cargo.toml"
    );
    let response = ureq::get(&url)
        .call()
        .map_err(|e| PdfToolError::new(format!("Failed to fetch remote version: {e}")))?;
    let body = response
        .into_string()
        .map_err(|e| PdfToolError::new(format!("Failed to read remote metadata: {e}")))?;

    extract_version_from_cargo_toml(&body).ok_or_else(|| {
        PdfToolError::new("Failed to parse remote version from Cargo.toml.")
    })
}

pub(crate) fn check_version_state(repo: &str, branch: &str) -> VersionState {
    let current = current_version().to_string();
    match fetch_remote_version(repo, branch) {
        Ok(latest) => {
            if compare_version_strings(&current, &latest) == Ordering::Less {
                VersionState::UpdateAvailable { current, latest }
            } else {
                VersionState::UpToDate { current }
            }
        }
        Err(_) => VersionState::Unknown { current },
    }
}

fn summarize_update_output(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let tail = text.lines().rev().take(8).collect::<Vec<_>>();
    if tail.is_empty() {
        "No command output available.".to_string()
    } else {
        tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
    }
}

pub fn update_multus(repo: &str, branch: &str) -> Result<()> {
    let output = Command::new("cargo")
        .arg("install")
        .arg("--git")
        .arg(repo)
        .arg("--branch")
        .arg(branch)
        .arg("--locked")
        .arg("--force")
        .arg("--bin")
        .arg("multus")
        .arg("-q")
        .output()
        .map_err(|e| {
            PdfToolError::new(format!(
                "Failed to run cargo update command. Ensure Rust/Cargo is installed and available in PATH: {e}"
            ))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let details = if !output.stderr.is_empty() {
        summarize_update_output(&output.stderr)
    } else {
        summarize_update_output(&output.stdout)
    };
    let code = output
        .status
        .code()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Err(PdfToolError::new(format!(
        "Update failed (exit code {code}). {details}"
    )))
}

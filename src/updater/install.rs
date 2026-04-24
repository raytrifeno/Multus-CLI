use std::process::Command;

use crate::types::{PdfToolError, Result};

use super::version::raw_github_script_url;

fn summarize_update_output(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let tail = text.lines().rev().take(8).collect::<Vec<_>>();
    if tail.is_empty() {
        "No command output available.".to_string()
    } else {
        tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
    }
}

fn update_with_cargo(repo: &str, branch: &str) -> Result<String> {
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
        return Ok("Update complete.".to_string());
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

#[cfg(windows)]
fn update_with_installer_script(script_url: &str) -> Result<String> {
    let url = super::shell::quote_powershell_literal(script_url);
    let script = format!(
        r#"
$url = {url}
Start-Sleep -Seconds 2
$installer = Invoke-RestMethod -Uri $url
Invoke-Expression $installer
"#
    );

    Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script)
        .spawn()
        .map_err(|e| {
            PdfToolError::new(format!(
                "Failed to start Windows update helper. Try the install.ps1 command from README instead: {e}"
            ))
        })?;

    Ok("Update started in a helper process. Wait a few seconds, then open a new terminal and run `multus --version`.".to_string())
}

#[cfg(not(windows))]
fn update_with_installer_script(script_url: &str) -> Result<String> {
    let command = format!(
        "sleep 2; curl -fsSL '{}' | bash",
        script_url.replace('\'', "'\\''")
    );
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .spawn()
        .map_err(|e| {
            PdfToolError::new(format!(
                "Failed to start update helper. Ensure curl and sh are available: {e}"
            ))
        })?;

    Ok("Update started in a helper process. Wait a few seconds, then open a new terminal and run `multus --version`.".to_string())
}

pub(crate) fn update_multus(repo: &str, branch: &str) -> Result<String> {
    let script_path = if cfg!(windows) {
        "scripts/install.ps1"
    } else {
        "scripts/install.sh"
    };

    if let Some(script_url) = raw_github_script_url(repo, branch, script_path) {
        return update_with_installer_script(&script_url);
    }

    update_with_cargo(repo, branch)
}

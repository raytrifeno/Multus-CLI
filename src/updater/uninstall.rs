use std::env;
#[cfg(not(windows))]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::types::{PdfToolError, Result};

fn current_executable_path() -> Result<PathBuf> {
    env::current_exe()
        .map_err(|e| PdfToolError::new(format!("Failed to locate current executable: {e}")))
}

fn current_executable_dir(exe_path: &Path) -> Result<PathBuf> {
    exe_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| PdfToolError::new("Failed to locate executable directory."))
}

#[cfg(windows)]
fn uninstall_windows(exe_path: &Path, exe_dir: &Path) -> Result<String> {
    let exe = super::shell::quote_powershell_literal(&exe_path.to_string_lossy());
    let dir = super::shell::quote_powershell_literal(&exe_dir.to_string_lossy());
    let script = format!(
        r#"
$exe = {exe}
$dir = {dir}
Start-Sleep -Seconds 2
Remove-Item -LiteralPath $exe -Force -ErrorAction SilentlyContinue
if (Test-Path -LiteralPath $dir -PathType Container) {{
    $remaining = Get-ChildItem -LiteralPath $dir -Force -ErrorAction SilentlyContinue
    if ($null -eq $remaining -or $remaining.Count -eq 0) {{
        Remove-Item -LiteralPath $dir -Force -ErrorAction SilentlyContinue
    }}
}}
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not [string]::IsNullOrWhiteSpace($userPath)) {{
    $normalizedDir = $dir.TrimEnd('\')
    $parts = @()
    foreach ($candidate in ($userPath -split ';')) {{
        if (-not [string]::IsNullOrWhiteSpace($candidate) -and $candidate.TrimEnd('\') -ine $normalizedDir) {{
            $parts += $candidate
        }}
    }}
    [Environment]::SetEnvironmentVariable("Path", ($parts -join ';'), "User")
}}
"#
    );

    Command::new("powershell")
        .arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-WindowStyle")
        .arg("Hidden")
        .arg("-Command")
        .arg(script)
        .spawn()
        .map_err(|e| {
            PdfToolError::new(format!(
                "Failed to start Windows uninstall helper. Try scripts\\uninstall.ps1 instead: {e}"
            ))
        })?;

    Ok("Uninstall scheduled. Close this terminal after the command exits, then open a new terminal.".to_string())
}

#[cfg(not(windows))]
fn remove_path_export_if_present(profile: &Path) -> Result<()> {
    if !profile.exists() {
        return Ok(());
    }

    let before = fs::read_to_string(profile).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to read PATH profile '{}': {e}",
            profile.display()
        ))
    })?;
    let mut after = String::new();
    let mut skip_next = false;
    for line in before.lines() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if line.trim() == "# Added by Multus installer" {
            skip_next = true;
            continue;
        }
        after.push_str(line);
        after.push('\n');
    }

    if after != before {
        fs::write(profile, after).map_err(|e| {
            PdfToolError::new(format!(
                "Failed to update PATH profile '{}': {e}",
                profile.display()
            ))
        })?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn uninstall_unix(exe_path: &Path) -> Result<String> {
    fs::remove_file(exe_path).map_err(|e| {
        PdfToolError::new(format!(
            "Failed to remove executable '{}': {e}",
            exe_path.display()
        ))
    })?;

    if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
        remove_path_export_if_present(&home.join(".profile"))?;
        remove_path_export_if_present(&home.join(".bashrc"))?;
        remove_path_export_if_present(&home.join(".zshrc"))?;
    }

    Ok(
        "Uninstall complete. If the current shell still remembers the old path, run `hash -r` or open a new shell."
            .to_string(),
    )
}

pub(crate) fn uninstall_multus() -> Result<String> {
    let exe_path = current_executable_path()?;
    let exe_dir = current_executable_dir(&exe_path)?;

    #[cfg(windows)]
    {
        uninstall_windows(&exe_path, &exe_dir)
    }

    #[cfg(not(windows))]
    {
        let _ = exe_dir;
        uninstall_unix(&exe_path)
    }
}

$ErrorActionPreference = "Stop"

function Write-Step {
    param([string]$Message)
    Write-Host "[multus-uninstall] $Message" -ForegroundColor Yellow
}

function Test-Command {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Confirm-Yes {
    param([string]$Prompt)

    $answer = Read-Host "$Prompt [y/N]"
    return $answer -match '^(y|yes)$'
}

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE ".cargo" }
$rustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE ".rustup" }

if (Test-Command "cargo") {
    Write-Step "Removing multus binary with cargo uninstall..."
    & cargo uninstall multus
    if ($LASTEXITCODE -ne 0) {
        Write-Step "cargo uninstall returned exit code $LASTEXITCODE (continuing cleanup)."
    }
}
else {
    Write-Step "cargo not found. Skipping cargo uninstall step."
}

if (Confirm-Yes "Remove downloaded Cargo package cache used by Multus (registry + git cache)?") {
    $registryPath = Join-Path $cargoHome "registry"
    $gitPath = Join-Path $cargoHome "git"
    Remove-Item -Path $registryPath -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $gitPath -Recurse -Force -ErrorAction SilentlyContinue
    Write-Step "Cargo cache removed."
}
else {
    Write-Step "Cargo cache kept."
}

if (Confirm-Yes "Also remove Rust installation (~/.rustup and ~/.cargo)?") {
    Remove-Item -Path $rustupHome -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $cargoHome -Recurse -Force -ErrorAction SilentlyContinue
    Write-Step "Rust toolchain and Cargo home removed."
}
else {
    Write-Step "Rust installation kept."
}

Write-Step "Uninstall complete."

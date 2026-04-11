$ErrorActionPreference = "Stop"

$repoUrl = "https://github.com/raytrifeno/scraks.git"
$repoRef = "main"

function Write-Step {
    param([string]$Message)
    Write-Host "[multus-install] $Message" -ForegroundColor Yellow
}

function Test-Command {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Ensure-Cargo {
    if (Test-Command "cargo") {
        Write-Step "Rust/Cargo detected. Skipping Rust installation."
        return
    }

    Write-Step "Rust/Cargo not found. Installing rustup..."
    $isArm64 = $env:PROCESSOR_ARCHITECTURE -eq "ARM64" -or $env:PROCESSOR_ARCHITEW6432 -eq "ARM64"
    $target = if ($isArm64) { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
    $rustupUrl = "https://static.rust-lang.org/rustup/dist/$target/rustup-init.exe"
    $rustupExe = Join-Path $env:TEMP "rustup-init.exe"

    Invoke-WebRequest -Uri $rustupUrl -OutFile $rustupExe
    & $rustupExe -y --profile minimal --default-toolchain stable
    if ($LASTEXITCODE -ne 0) {
        throw "rustup installation failed with exit code $LASTEXITCODE."
    }

    Remove-Item -Path $rustupExe -Force -ErrorAction SilentlyContinue

    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargoBin -PathType Container) {
        $pathEntries = $env:Path -split ";" | ForEach-Object { $_.TrimEnd('\') }
        if (-not ($pathEntries -contains $cargoBin.TrimEnd('\'))) {
            $env:Path += ";$cargoBin"
        }
    }

    if (-not (Test-Command "cargo")) {
        throw "Cargo is not available yet. Open a new terminal, then run the installer again."
    }
}

Ensure-Cargo

Write-Step "Installing multus from $repoUrl (ref: $repoRef)..."
& cargo install --git $repoUrl --branch $repoRef --force --locked --bin multus
if ($LASTEXITCODE -ne 0) {
    throw "cargo install failed with exit code $LASTEXITCODE. Ensure $repoUrl (ref: $repoRef) contains Cargo.toml and binary target 'multus'."
}

if (Test-Command "multus") {
    Write-Step "Installation complete."
    multus --help
} else {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    Write-Step "Installed, but 'multus' is not on PATH in this session."
    Write-Host "Add this path and reopen terminal: $cargoBin"
}

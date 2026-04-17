$ErrorActionPreference = "Stop"

$installDir = Join-Path $env:LOCALAPPDATA "Programs\Multus\bin"
$binaryPath = Join-Path $installDir "multus.exe"

function Write-Step {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Yellow
}

function Normalize-PathEntry {
    param([string]$PathEntry)
    if ([string]::IsNullOrWhiteSpace($PathEntry)) {
        return ""
    }
    return $PathEntry.Trim().TrimEnd('\')
}

function Remove-PathEntry {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    $normalizedEntry = Normalize-PathEntry -PathEntry $Entry
    if ([string]::IsNullOrWhiteSpace($normalizedEntry) -or [string]::IsNullOrWhiteSpace($PathValue)) {
        return $PathValue
    }

    $parts = @()
    foreach ($candidate in ($PathValue -split ';')) {
        if ((Normalize-PathEntry -PathEntry $candidate) -ine $normalizedEntry) {
            if (-not [string]::IsNullOrWhiteSpace($candidate)) {
                $parts += $candidate
            }
        }
    }
    return ($parts -join ';')
}

if (Test-Path $binaryPath -PathType Leaf) {
    Remove-Item -Path $binaryPath -Force
    Write-Step "Removed: $binaryPath"
}
else {
    Write-Step "Binary not found at: $binaryPath"
}

if (Test-Path $installDir -PathType Container) {
    $remaining = Get-ChildItem -Path $installDir -Force -ErrorAction SilentlyContinue
    if ($null -eq $remaining -or $remaining.Count -eq 0) {
        Remove-Item -Path $installDir -Force
        Write-Step "Removed empty directory: $installDir"
    }
}

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$newUserPath = Remove-PathEntry -PathValue $userPath -Entry $installDir
if ($newUserPath -ne $userPath) {
    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
    Write-Step "Removed install directory from user PATH."
}

$sessionPath = $env:Path
$newSessionPath = Remove-PathEntry -PathValue $sessionPath -Entry $installDir
if ($newSessionPath -ne $sessionPath) {
    $env:Path = $newSessionPath
    Write-Step "Updated PATH for current session."
}

Write-Step "Uninstall complete."

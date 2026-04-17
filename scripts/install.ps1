param(
    [switch]$DryRun,
    [ValidateSet("auto", "interactive", "compact")]
    [string]$UiMode = "auto"
)

$ErrorActionPreference = "Stop"

$repoOwner = "raytrifeno"
$repoName = "Multus-CLI"
$binaryName = "multus.exe"
$assetName = "multus-windows-x64.zip"
$downloadUrl = "https://github.com/$repoOwner/$repoName/releases/latest/download/$assetName"
$installDir = Join-Path $env:LOCALAPPDATA "Programs\Multus\bin"
$installPath = Join-Path $installDir $binaryName

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

function Path-ContainsEntry {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    $normalizedEntry = Normalize-PathEntry -PathEntry $Entry
    if ([string]::IsNullOrWhiteSpace($normalizedEntry) -or [string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    foreach ($candidate in ($PathValue -split ';')) {
        if ((Normalize-PathEntry -PathEntry $candidate) -ieq $normalizedEntry) {
            return $true
        }
    }
    return $false
}

function Ensure-InstallPathInUserPath {
    param([string]$PathToAdd)

    $updatedUserPath = $false
    $updatedSessionPath = $false

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not (Path-ContainsEntry -PathValue $userPath -Entry $PathToAdd)) {
        $newUserPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
            $PathToAdd
        }
        else {
            "$userPath;$PathToAdd"
        }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
        $updatedUserPath = $true
    }

    if (-not (Path-ContainsEntry -PathValue $env:Path -Entry $PathToAdd)) {
        $env:Path = if ([string]::IsNullOrWhiteSpace($env:Path)) {
            $PathToAdd
        }
        else {
            "$env:Path;$PathToAdd"
        }
        $updatedSessionPath = $true
    }

    return @{
        UpdatedUserPath = $updatedUserPath
        UpdatedSessionPath = $updatedSessionPath
    }
}

Write-Step "Release asset: $assetName"
Write-Step "Download URL: $downloadUrl"
Write-Step "Install directory: $installDir"
Write-Step "UI mode: $UiMode"

if ($DryRun) {
    Write-Step "Dry-run finished. No files were changed."
    exit 0
}

$tempRoot = Join-Path $env:TEMP ("multus-install-" + [Guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $tempRoot $assetName
$extractDir = Join-Path $tempRoot "extract"

try {
    New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
    New-Item -ItemType Directory -Path $extractDir -Force | Out-Null
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null

    Write-Step "Downloading latest release binary..."
    Invoke-WebRequest -Uri $downloadUrl -OutFile $archivePath -UseBasicParsing

    Write-Step "Extracting archive..."
    Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force

    $extractedBinary = Join-Path $extractDir $binaryName
    if (-not (Test-Path $extractedBinary -PathType Leaf)) {
        throw "Archive did not contain $binaryName."
    }

    Copy-Item -Path $extractedBinary -Destination $installPath -Force

    $pathState = Ensure-InstallPathInUserPath -PathToAdd $installDir
    if ($pathState.UpdatedUserPath) {
        Write-Step "Added install directory to user PATH."
    }
    if ($pathState.UpdatedSessionPath) {
        Write-Step "Updated PATH for current session."
    }

    Write-Step "Installation complete."
    if (Get-Command multus -ErrorAction SilentlyContinue) {
        & multus --help | Out-Null
        Write-Step "Command available: multus"
    }
    else {
        Write-Step "Binary installed to: $installPath"
        Write-Step "Open a new terminal if command is not yet in PATH."
    }
}
finally {
    if (Test-Path $tempRoot -PathType Container) {
        Remove-Item -Path $tempRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

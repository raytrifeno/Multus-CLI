$ErrorActionPreference = "Stop"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$repoUrl = "https://github.com/raytrifeno/scraks.git"
$repoRef = "main"
$maxVisible = 10
$maxActive = 3

function Write-Step {
    param([string]$Message)
    Write-Host "[multus-install] $Message" -ForegroundColor Yellow
}

function Test-Command {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Ensure-Prerequisites {
    if (-not (Test-Command "git")) {
        throw "git is required. Install git first, then run this installer again."
    }

    if (-not (Test-Command "cargo")) {
        throw "Rust/Cargo not found. Install Rust first at https://www.rust-lang.org/tools/install, then rerun this installer."
    }
}

function Get-LockPackages {
    param([string]$LockPath)

    if (-not (Test-Path $LockPath -PathType Leaf)) {
        return @()
    }

    $seen = @{}
    $packages = New-Object System.Collections.Generic.List[string]
    foreach ($line in Get-Content -Path $LockPath) {
        if ($line -match '^name = "([^"]+)"') {
            $name = $matches[1]
            if (-not $seen.ContainsKey($name)) {
                $seen[$name] = $true
                $packages.Add($name)
            }
        }
    }

    return ,$packages.ToArray()
}

function New-StageState {
    param([string[]]$Tasks)

    if (-not $Tasks -or $Tasks.Count -eq 0) {
        $Tasks = @("multus")
    }

    $pending = [System.Collections.Generic.Queue[string]]::new()
    foreach ($task in $Tasks) {
        $pending.Enqueue($task)
    }

    $active = [System.Collections.Generic.List[string]]::new()
    while ($active.Count -lt $maxActive -and $pending.Count -gt 0) {
        $active.Add($pending.Dequeue())
    }

    return @{
        Total   = $Tasks.Count
        Done    = 0
        Pending = $pending
        Active  = $active
    }
}

function Advance-StageState {
    param([hashtable]$State)

    if ($State.Active.Count -gt 0) {
        $State.Active.RemoveAt(0)
    }

    if ($State.Done -lt $State.Total) {
        $State.Done++
    }

    if ($State.Active.Count -lt $maxActive -and $State.Pending.Count -gt 0) {
        $State.Active.Add($State.Pending.Dequeue())
    }
}

function Render-ParallelUi {
    param(
        [string]$Title,
        [hashtable]$State
    )

    Clear-Host
    Write-Host "[multus-install] $Title" -ForegroundColor Yellow
    Write-Host "Progress: $($State.Done)/$($State.Total) | Active: $($State.Active.Count)/$maxActive | Visible: $maxVisible"
    Write-Host ""

    $shown = 0
    foreach ($task in $State.Active) {
        if ($shown -ge $maxVisible) {
            break
        }
        Write-Host ("  [RUNNING] {0}" -f $task) -ForegroundColor Cyan
        $shown++
    }

    foreach ($task in $State.Pending.ToArray()) {
        if ($shown -ge $maxVisible) {
            break
        }
        Write-Host ("  [QUEUED ] {0}" -f $task) -ForegroundColor DarkGray
        $shown++
    }

    if ($shown -eq 0) {
        Write-Host "  waiting for events..." -ForegroundColor DarkGray
    }
}

function Join-CmdArgLine {
    param([string[]]$Args)

    $encoded = foreach ($arg in $Args) {
        if ($arg -match '[\s"]') {
            '"' + $arg.Replace('"', '\"') + '"'
        }
        else {
            $arg
        }
    }
    return ($encoded -join " ")
}

function Invoke-CargoStage {
    param(
        [string]$Title,
        [string[]]$Tasks,
        [string]$EventRegex,
        [string[]]$CargoArgs,
        [string]$WorkingDir
    )

    $state = New-StageState -Tasks $Tasks
    Render-ParallelUi -Title $Title -State $state

    Push-Location $WorkingDir
    try {
        $argLine = Join-CmdArgLine -Args $CargoArgs
        & cmd /d /c "cargo $argLine 2>&1" | ForEach-Object {
            $line = $_.ToString()
            if ($line -match $EventRegex) {
                Advance-StageState -State $state
                Render-ParallelUi -Title $Title -State $state
            }
        }

        if ($LASTEXITCODE -ne 0) {
            throw "cargo $($CargoArgs -join ' ') failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        Pop-Location
    }

    while ($state.Done -lt $state.Total) {
        Advance-StageState -State $state
    }
    Render-ParallelUi -Title $Title -State $state
    Write-Host ""
    Write-Step "$Title complete."
}

Ensure-Prerequisites

$workDir = Join-Path $env:TEMP ("multus-install-" + [Guid]::NewGuid().ToString("N"))

try {
    Write-Step "Cloning $repoUrl (ref: $repoRef)..."
    & git clone --depth 1 --branch $repoRef $repoUrl $workDir | Out-Null
    if ($LASTEXITCODE -ne 0) {
        throw "git clone failed with exit code $LASTEXITCODE."
    }

    $manifestPath = Join-Path $workDir "Cargo.toml"
    if (-not (Test-Path $manifestPath -PathType Leaf)) {
        throw "Cargo.toml not found in repository. Check $repoUrl (ref: $repoRef)."
    }

    $lockPath = Join-Path $workDir "Cargo.lock"
    if (-not (Test-Path $lockPath -PathType Leaf)) {
        Write-Step "Cargo.lock not found. Generating lockfile..."
        Push-Location $workDir
        try {
            & cargo generate-lockfile
            if ($LASTEXITCODE -ne 0) {
                throw "cargo generate-lockfile failed with exit code $LASTEXITCODE."
            }
        }
        finally {
            Pop-Location
        }
    }

    $packages = Get-LockPackages -LockPath $lockPath
    if (-not $packages -or $packages.Count -eq 0) {
        $packages = @("dependencies")
    }

    Invoke-CargoStage `
        -Title "Downloading crates (parallel task runner)" `
        -Tasks $packages `
        -EventRegex 'Downloaded\s+[^\s]+' `
        -CargoArgs @("fetch", "--locked", "--manifest-path", $manifestPath, "-vv") `
        -WorkingDir $workDir

    $compileTasks = @($packages + @("multus"))
    Invoke-CargoStage `
        -Title "Compiling crates (parallel task runner)" `
        -Tasks $compileTasks `
        -EventRegex 'Compiling\s+[^\s]+' `
        -CargoArgs @("install", "--path", $workDir, "--locked", "--force", "--bin", "multus", "-j", "$maxActive") `
        -WorkingDir $workDir
}
finally {
    if (Test-Path $workDir -PathType Container) {
        Remove-Item -Path $workDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

if (Test-Command "multus") {
    Write-Step "Installation complete."
    multus --help
}
else {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    Write-Step "Installed, but 'multus' is not on PATH in this session."
    Write-Host "Open a new terminal or add this path manually: $cargoBin"
}

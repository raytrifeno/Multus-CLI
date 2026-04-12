param(
    [switch]$DryRun,
    [ValidateSet("auto", "interactive", "compact")]
    [string]$UiMode = "auto"
)

$ErrorActionPreference = "Stop"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$repoUrl = "https://github.com/raytrifeno/scraks.git"
$repoRef = "main"
$maxDisplay = 10
$maxActive = 3
$script:lastFrameLineCount = 0

function Write-Step {
    param([string]$Message)
    Write-Host "[multus-install] $Message" -ForegroundColor Yellow
}

function Test-Command {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Resolve-UiMode {
    if ($UiMode -ne "auto") {
        return $UiMode
    }

    if (-not [Console]::IsOutputRedirected -and $Host.Name -eq "ConsoleHost") {
        return "interactive"
    }

    return "compact"
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

    $completed = ""
    if ($State.Active.Count -gt 0) {
        $completed = $State.Active[0]
        $State.Active.RemoveAt(0)
    }

    if ($State.Done -lt $State.Total) {
        $State.Done++
    }

    if ($State.Active.Count -lt $maxActive -and $State.Pending.Count -gt 0) {
        $State.Active.Add($State.Pending.Dequeue())
    }

    return $completed
}

function New-ProgressBar {
    param(
        [int]$Done,
        [int]$Total
    )

    $width = 24
    if ($Total -le 0) {
        $Total = 1
    }

    $ratio = [Math]::Min(1.0, [Math]::Max(0.0, $Done / $Total))
    $filled = [Math]::Floor($ratio * $width)
    $empty = $width - $filled

    return ("[" + ("#" * $filled) + ("-" * $empty) + "]")
}

function Build-FrameLines {
    param(
        [string]$Title,
        [hashtable]$State,
        [string]$CompletedTask
    )

    $bar = New-ProgressBar -Done $State.Done -Total $State.Total
    $lines = New-Object System.Collections.Generic.List[string]
    $lines.Add("[multus-install] $Title")
    $lines.Add("Progress $bar $($State.Done)/$($State.Total) | Active: $($State.Active.Count)/$maxActive")

    if ($CompletedTask) {
        $lines.Add("Completed: $CompletedTask")
    }

    $shown = 0
    foreach ($task in $State.Active) {
        if ($shown -ge $maxDisplay) {
            break
        }
        $lines.Add(("  [RUNNING] {0}" -f $task))
        $shown++
    }

    foreach ($task in $State.Pending.ToArray()) {
        if ($shown -ge $maxDisplay) {
            break
        }
        $lines.Add(("  [QUEUED ] {0}" -f $task))
        $shown++
    }

    if ($shown -eq 0) {
        $lines.Add("  waiting for events...")
    }

    return ,$lines.ToArray()
}

function Render-Stage {
    param(
        [string]$Mode,
        [string]$Title,
        [hashtable]$State,
        [string]$CompletedTask,
        [switch]$Final
    )

    if ($Mode -eq "interactive") {
        $lines = Build-FrameLines -Title $Title -State $State -CompletedTask $CompletedTask
        if ($script:lastFrameLineCount -gt 0) {
            Write-Host ("`e[{0}A" -f $script:lastFrameLineCount) -NoNewline
        }
        foreach ($line in $lines) {
            Write-Host ("`e[2K{0}" -f $line)
        }
        $script:lastFrameLineCount = $lines.Count
        if ($Final) {
            Write-Host ""
            $script:lastFrameLineCount = 0
        }
        return
    }

    $bar = New-ProgressBar -Done $State.Done -Total $State.Total
    $activePreview = ($State.Active | Select-Object -First $maxActive) -join ", "
    if ([string]::IsNullOrWhiteSpace($activePreview)) {
        $activePreview = "none"
    }

    $nextLimit = [Math]::Max(0, $maxDisplay - [Math]::Min($maxActive, $State.Active.Count))
    $nextPreview = ($State.Pending.ToArray() | Select-Object -First $nextLimit) -join ", "
    if ([string]::IsNullOrWhiteSpace($nextPreview)) {
        $nextPreview = "none"
    }

    $doneLabel = if ([string]::IsNullOrWhiteSpace($CompletedTask)) { "-" } else { $CompletedTask }
    Write-Host ("[multus-install] {0} | {1} {2}/{3} | done: {4} | active: {5} | next: {6}" -f $Title, $bar, $State.Done, $State.Total, $doneLabel, $activePreview, $nextPreview)
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

function Invoke-SimulatedStage {
    param(
        [string]$Mode,
        [string]$Title,
        [string[]]$Tasks
    )

    $state = New-StageState -Tasks $Tasks
    Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask ""
    while ($state.Done -lt $state.Total) {
        $completed = Advance-StageState -State $state
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask $completed
    }
    Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask "" -Final
    Write-Step "$Title complete."
}

function Invoke-CargoStage {
    param(
        [string]$Mode,
        [string]$Title,
        [string[]]$Tasks,
        [string]$EventRegex,
        [string[]]$CargoArgs,
        [string]$WorkingDir
    )

    $state = New-StageState -Tasks $Tasks
    Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask ""

    Push-Location $WorkingDir
    try {
        $argLine = Join-CmdArgLine -Args $CargoArgs
        & cmd /d /c "cargo $argLine 2>&1" | ForEach-Object {
            $line = $_.ToString()
            if ($line -match $EventRegex) {
                $completed = Advance-StageState -State $state
                Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask $completed
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
        $completed = Advance-StageState -State $state
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask $completed
    }

    Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask "" -Final
    Write-Step "$Title complete."
}

$renderMode = Resolve-UiMode

if ($DryRun) {
    $packages = 1..12 | ForEach-Object { "crate-{0:00}" -f $_ }
    Invoke-SimulatedStage -Mode $renderMode -Title "Downloading crates (parallel task runner)" -Tasks $packages
    Invoke-SimulatedStage -Mode $renderMode -Title "Compiling crates (parallel task runner)" -Tasks (@($packages + @("multus")))
    Write-Step "Dry-run finished. No installation was performed."
    exit 0
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
        -Mode $renderMode `
        -Title "Downloading crates (parallel task runner)" `
        -Tasks $packages `
        -EventRegex 'Downloaded\s+[^\s]+' `
        -CargoArgs @("fetch", "--locked", "--manifest-path", $manifestPath, "-vv") `
        -WorkingDir $workDir

    $compileTasks = @($packages + @("multus"))
    Invoke-CargoStage `
        -Mode $renderMode `
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

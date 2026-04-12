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
$ansiEsc = [char]27
$ansiReset = "$ansiEsc[0m"
$ansiGreen = "$ansiEsc[32m"
$ansiOrange = "$ansiEsc[38;5;208m"

function Write-Step {
    param([string]$Message)
    Write-Host $Message -ForegroundColor Yellow
}

function Test-Command {
    param([string]$Name)
    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Normalize-PathEntry {
    param([string]$PathEntry)

    if ([string]::IsNullOrWhiteSpace($PathEntry)) {
        return ""
    }

    return $PathEntry.Trim().TrimEnd('\\')
}

function Path-ContainsEntry {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    $normalizedEntry = Normalize-PathEntry -PathEntry $Entry
    if ([string]::IsNullOrWhiteSpace($normalizedEntry)) {
        return $false
    }
    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return $false
    }

    foreach ($candidate in ($PathValue -split ';')) {
        if ((Normalize-PathEntry -PathEntry $candidate) -ieq $normalizedEntry) {
            return $true
        }
    }

    return $false
}

function Ensure-CargoBinOnPath {
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    $result = @{
        CargoBin         = $cargoBin
        AddedUserPath    = $false
        AddedSessionPath = $false
    }

    if (-not (Test-Path $cargoBin -PathType Container)) {
        return $result
    }

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not (Path-ContainsEntry -PathValue $userPath -Entry $cargoBin)) {
        $newUserPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
            $cargoBin
        }
        else {
            "$userPath;$cargoBin"
        }
        [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
        $result.AddedUserPath = $true
    }

    if (-not (Path-ContainsEntry -PathValue $env:Path -Entry $cargoBin)) {
        $env:Path = if ([string]::IsNullOrWhiteSpace($env:Path)) {
            $cargoBin
        }
        else {
            "$env:Path;$cargoBin"
        }
        $result.AddedSessionPath = $true
    }

    return $result
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

function Install-RustToolchain {
    Write-Step "Rust/Cargo not found. Installing Rust toolchain..."

    $rustupTempDir = Join-Path $env:TEMP ("multus-rustup-" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $rustupTempDir -Force | Out-Null
    $rustupExe = Join-Path $rustupTempDir "rustup-init.exe"

    try {
        Invoke-SimulatedStage -Mode $renderMode -Title "Downloading" -Tasks @("rustup-init")
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupExe -UseBasicParsing

        $rustTasks = @("channel", "cargo", "clippy", "rust-docs", "rust-std", "rustc", "rustfmt")
        Invoke-ProcessStage `
            -Mode $renderMode `
            -Title "Compiling" `
            -Tasks $rustTasks `
            -EventRegex 'downloading component|installing component|syncing channel updates|default toolchain set to' `
            -Executable $rustupExe `
            -Arguments @("-y", "--profile", "default", "--default-toolchain", "stable")
    }
    finally {
        if (Test-Path $rustupTempDir -PathType Container) {
            Remove-Item -Path $rustupTempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    $pathState = Ensure-CargoBinOnPath
    if ($pathState.AddedUserPath) {
        Write-Step "Added '$($pathState.CargoBin)' to user PATH."
    }
    if ($pathState.AddedSessionPath) {
        Write-Step "Updated PATH for current session."
    }

    if (-not (Test-Command "cargo")) {
        throw "Rust/Cargo installation failed. Install manually from https://www.rust-lang.org/tools/install then rerun installer."
    }
}

function Ensure-Prerequisites {
    if (-not (Test-Command "git")) {
        throw "git is required. Install git first, then run this installer again."
    }

    if (-not (Test-Command "cargo")) {
        Install-RustToolchain
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
    param(
        [string[]]$Tasks,
        [string]$UnitLabel = "count",
        [int]$TotalUnits = 0
    )

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
        Total      = $Tasks.Count
        Done       = 0
        Pending    = $pending
        Active     = $active
        UnitLabel  = $UnitLabel
        TotalUnits = $(if ($TotalUnits -gt 0) { $TotalUnits } else { $Tasks.Count })
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

    if ($Done -le 0) {
        return ("[>" + ("-" * ($width - 1)) + "]")
    }

    if ($Done -ge $Total) {
        return ("[" + ("=" * $width) + "]")
    }

    $filled = [Math]::Floor(($Done / [double]$Total) * ($width - 1))
    if ($filled -lt 2) {
        $filled = 2
    }
    if ($filled -ge $width) {
        $filled = $width - 1
    }

    $empty = $width - $filled - 1
    return ("[" + ("=" * $filled) + ">" + ("-" * $empty) + "]")
}

function Get-DisplayProgress {
    param([hashtable]$State)

    if ($State.UnitLabel -eq "MB") {
        $totalUnits = [Math]::Max(1, [int]$State.TotalUnits)
        $doneUnits = if ($State.Done -ge $State.Total) {
            $totalUnits
        }
        elseif ($State.Total -le 0) {
            0
        }
        else {
            [Math]::Floor(($State.Done / [double]$State.Total) * $totalUnits)
        }

        return @{
            Done   = [int]$doneUnits
            Total  = $totalUnits
            Suffix = " MB"
        }
    }

    return @{
        Done   = [int]$State.Done
        Total  = [Math]::Max(1, [int]$State.Total)
        Suffix = ""
    }
}

function Get-StageVerb {
    param([string]$Title)

    if ($Title -match '^\s*Compiling') {
        return "${ansiOrange}Compiling${ansiReset}"
    }

    return "${ansiGreen}Downloading${ansiReset}"
}

function Get-StageTask {
    param(
        [hashtable]$State,
        [string]$CompletedTask
    )

    if (-not [string]::IsNullOrWhiteSpace($CompletedTask)) {
        return $CompletedTask
    }

    if ($State.Active.Count -gt 0) {
        return $State.Active[0]
    }

    if ($State.Pending.Count -gt 0) {
        return $State.Pending.Peek()
    }

    return "multus"
}

function Get-StageSummaryVerb {
    param([string]$Title)

    if ($Title -match '^\s*Compiling') {
        return "compile"
    }

    return "download"
}

function Write-StageSummary {
    param(
        [string]$Title,
        [hashtable]$State
    )

    $verb = Get-StageSummaryVerb -Title $Title
    Write-Host ("{0} {1}/{2}" -f $verb, $State.Done, $State.Total)
    Write-Host ""
}

function Format-LoadingBar {
    param([string]$Bar)

    return "${ansiOrange}${Bar}${ansiReset}"
}

function Build-FrameLines {
    param(
        [string]$Title,
        [hashtable]$State,
        [string]$CompletedTask
    )

    $progress = Get-DisplayProgress -State $State
    $bar = New-ProgressBar -Done $progress.Done -Total $progress.Total
    $coloredBar = Format-LoadingBar -Bar $bar
    $verb = Get-StageVerb -Title $Title
    $task = Get-StageTask -State $State -CompletedTask $CompletedTask

    $line = "{0} {1} | {2} {3}/{4}{5}" -f $verb, $task, $coloredBar, $progress.Done, $progress.Total, $progress.Suffix
    return ,@($line)
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

    $progress = Get-DisplayProgress -State $State
    $bar = New-ProgressBar -Done $progress.Done -Total $progress.Total
    $coloredBar = Format-LoadingBar -Bar $bar
    $verb = Get-StageVerb -Title $Title
    $task = Get-StageTask -State $State -CompletedTask $CompletedTask
    Write-Host ("{0} {1} | {2} {3}/{4}{5}" -f $verb, $task, $coloredBar, $progress.Done, $progress.Total, $progress.Suffix)
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
        [string[]]$Tasks,
        [string]$UnitLabel = "count",
        [int]$TotalUnits = 0
    )

    $state = New-StageState -Tasks $Tasks -UnitLabel $UnitLabel -TotalUnits $TotalUnits
    if ($Mode -ne "interactive") {
        Write-StageSummary -Title $Title -State $state
    }
    Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask ""
    while ($state.Done -lt $state.Total) {
        $completed = Advance-StageState -State $state
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask $completed
    }
    if ($Mode -eq "interactive") {
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask "" -Final
    }
    Write-Step "$Title complete."
}

function Invoke-CargoStage {
    param(
        [string]$Mode,
        [string]$Title,
        [string[]]$Tasks,
        [string]$EventRegex,
        [string[]]$CargoArgs,
        [string]$WorkingDir,
        [string]$UnitLabel = "count",
        [int]$TotalUnits = 0
    )

    $state = New-StageState -Tasks $Tasks -UnitLabel $UnitLabel -TotalUnits $TotalUnits
    if ($Mode -ne "interactive") {
        Write-StageSummary -Title $Title -State $state
    }
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

    if ($Mode -eq "interactive") {
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask "" -Final
    }
    Write-Step "$Title complete."
}

function Invoke-ProcessStage {
    param(
        [string]$Mode,
        [string]$Title,
        [string[]]$Tasks,
        [string]$EventRegex,
        [string]$Executable,
        [string[]]$Arguments,
        [string]$WorkingDir = "",
        [string]$UnitLabel = "count",
        [int]$TotalUnits = 0
    )

    $state = New-StageState -Tasks $Tasks -UnitLabel $UnitLabel -TotalUnits $TotalUnits
    if ($Mode -ne "interactive") {
        Write-StageSummary -Title $Title -State $state
    }
    Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask ""

    $pushed = $false
    if (-not [string]::IsNullOrWhiteSpace($WorkingDir)) {
        Push-Location $WorkingDir
        $pushed = $true
    }

    try {
        & $Executable @Arguments 2>&1 | ForEach-Object {
            $line = $_.ToString()
            if ($line -match $EventRegex) {
                $completed = Advance-StageState -State $state
                Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask $completed
            }
        }

        if ($LASTEXITCODE -ne 0) {
            throw "$Executable $($Arguments -join ' ') failed with exit code $LASTEXITCODE."
        }
    }
    finally {
        if ($pushed) {
            Pop-Location
        }
    }

    while ($state.Done -lt $state.Total) {
        $completed = Advance-StageState -State $state
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask $completed
    }

    if ($Mode -eq "interactive") {
        Render-Stage -Mode $Mode -Title $Title -State $state -CompletedTask "" -Final
    }
    Write-Step "$Title complete."
}

$renderMode = Resolve-UiMode

if ($DryRun) {
    $packages = 1..12 | ForEach-Object { "package-{0:00}" -f $_ }
    $dryDownloadMb = [int]($packages.Count * 12)
    Invoke-SimulatedStage -Mode $renderMode -Title "Downloading" -Tasks $packages -UnitLabel "MB" -TotalUnits $dryDownloadMb
    Invoke-SimulatedStage -Mode $renderMode -Title "Compiling" -Tasks (@($packages + @("multus")))
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

    $downloadTotalMb = [int][Math]::Max(64, [Math]::Round($packages.Count * 6))

    Invoke-CargoStage `
        -Mode $renderMode `
        -Title "Downloading" `
        -Tasks $packages `
        -EventRegex 'Downloaded\s+[^\s]+' `
        -CargoArgs @("fetch", "--locked", "--manifest-path", $manifestPath, "-vv") `
        -WorkingDir $workDir `
        -UnitLabel "MB" `
        -TotalUnits $downloadTotalMb

    $compileTasks = @($packages + @("multus"))
    Invoke-CargoStage `
        -Mode $renderMode `
        -Title "Compiling" `
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

$pathState = Ensure-CargoBinOnPath
if ($pathState.AddedUserPath) {
    Write-Step "Added '$($pathState.CargoBin)' to user PATH."
}
if ($pathState.AddedSessionPath) {
    Write-Step "Updated PATH for current session."
}

if (Test-Command "multus") {
    Write-Step "Installation complete."
    multus --help
}
else {
    Write-Step "Installed, but 'multus' is still not detected in this session."
    Write-Host "Try opening a new terminal. Expected binary location: $($pathState.CargoBin)"
}

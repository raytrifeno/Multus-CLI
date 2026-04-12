$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$installScript = Join-Path $PSScriptRoot "install.ps1"

if (-not (Test-Path $installScript -PathType Leaf)) {
    throw "install.ps1 not found at: $installScript"
}

$output = & powershell -NoProfile -ExecutionPolicy Bypass -File $installScript -DryRun -UiMode compact 2>&1 | Out-String

if ($output -notmatch "Downloading crates \(parallel task runner\)") {
    throw "Download stage title not found in dry-run output."
}

if ($output -notmatch "Compiling crates \(parallel task runner\)") {
    throw "Compile stage title not found in dry-run output."
}

if ($output -notmatch "\[[#-]+\]\s+\d+/\d+") {
    throw "Progress bar output not found."
}

if ($output -match "Visible:") {
    throw "Unexpected 'Visible:' text found in output."
}

if ($output -notmatch "done:\s+crate-01") {
    throw "Expected first completed crate event not found."
}

if ($output -notmatch "Dry-run finished\. No installation was performed\.") {
    throw "Dry-run completion message not found."
}

Write-Host "[multus-test] install UI dry-run test passed."

# One-command Windows setup: toolchain, sidecars, npm install. Does not launch the GUI.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

if (-not (Test-Path (Join-Path $Root "vendor\sunshine\CMakeLists.txt")) -or
    -not (Test-Path (Join-Path $Root "vendor\moonlight-qt\moonlight-qt.pro"))) {
    Write-Host "Initializing git submodules..."
    git submodule update --init --recursive
}

& (Join-Path $PSScriptRoot "bootstrap.ps1")
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

. (Join-Path $PSScriptRoot "windows-dev-env.ps1")

& (Join-Path $PSScriptRoot "build-sidecars.ps1")
npm install

Write-Host ""
Write-Host "Setup complete. Start the app with:"
Write-Host "  npm run tauri -- dev"

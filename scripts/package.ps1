# Build the Windows NSIS installer. CUDA toolkit is not bundled; NVENC uses the Game Ready driver.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

. (Join-Path $PSScriptRoot "windows-dev-env.ps1")

if ($args -contains "-SkipSidecars") {
    Write-Host "==> Skipping sidecar rebuild"
} else {
    Write-Host "==> Building sidecars"
    & (Join-Path $PSScriptRoot "build-sidecars.ps1")
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "build-sidecars.ps1 exited $LASTEXITCODE"
    }
}

if (-not (Test-Path (Join-Path $Root "node_modules"))) {
    npm install
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "npm install exited $LASTEXITCODE"
    }
}

$svc = Join-Path $Root "src-tauri\resources\windows\sunshinesvc.exe"
if (-not (Test-Path $svc)) {
    throw "sunshinesvc.exe missing at $svc. Re-run scripts\build-sidecars.ps1 (or omit -SkipSidecars)."
}

Write-Host "==> Bundling NSIS installer"
npm run tauri -- build --bundles nsis
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "tauri build exited $LASTEXITCODE"
}

$nsis = Get-ChildItem -Path (Join-Path $Root "src-tauri\target\release\bundle\nsis") -Filter "*.exe" -ErrorAction SilentlyContinue
if ($nsis) {
    Write-Host ""
    Write-Host "Windows installer:"
    $nsis | ForEach-Object { Write-Host $_.FullName }
} else {
    Write-Host "Look for the setup exe under src-tauri\target\release\bundle\nsis"
}

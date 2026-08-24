# One-command Windows setup: toolchain, sidecars, npm install. Does not launch the GUI.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$EclipseLog = Join-Path $env:TEMP "eclipse-windows-setup.log"
Set-Location $Root

function Test-ShouldPauseOnError {
    if ($env:ECLIPSE_NO_PAUSE) { return $false }
    if ($env:CI) { return $false }
    try {
        $parentId = (Get-CimInstance Win32_Process -Filter "ProcessId=$PID" -ErrorAction Stop).ParentProcessId
        $parent = (Get-CimInstance Win32_Process -Filter "ProcessId=$parentId" -ErrorAction Stop).Name
        return $parent -match "explorer|Consent|ApplicationFrameHost|sihost|svchost"
    } catch {
        return $false
    }
}

function Show-EclipseFailure {
    param([string]$Message)
    Write-Host ""
    Write-Host $Message -ForegroundColor Red
    Write-Host "Log: $EclipseLog"
    if (Test-ShouldPauseOnError) {
        Write-Host "Press Enter to close this window."
        try { Read-Host | Out-Null } catch { }
    }
}

try {
    if (-not (Test-Path (Join-Path $Root "vendor\sunshine\CMakeLists.txt")) -or
        -not (Test-Path (Join-Path $Root "vendor\moonlight-qt\moonlight-qt.pro"))) {
        Write-Host "Initializing git submodules..."
        git submodule update --init --recursive
    }

    & (Join-Path $PSScriptRoot "bootstrap.ps1")
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "bootstrap.ps1 exited $LASTEXITCODE"
    }

    . (Join-Path $PSScriptRoot "windows-dev-env.ps1")

    & (Join-Path $PSScriptRoot "build-sidecars.ps1")
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "build-sidecars.ps1 exited $LASTEXITCODE"
    }
    npm install
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        throw "npm install exited $LASTEXITCODE"
    }

    Write-Host ""
    Write-Host "Setup complete. Start the app with:"
    Write-Host "  npm run tauri -- dev"
} catch {
    Show-EclipseFailure $_.Exception.Message
    exit 1
}

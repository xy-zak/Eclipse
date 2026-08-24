# Build Sunshine and Moonlight sidecars on Windows 11.
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "windows-dev-env.ps1")
$Root = Split-Path -Parent $PSScriptRoot
$SunshineSrc = Join-Path $Root "vendor\sunshine"
$MoonlightSrc = Join-Path $Root "vendor\moonlight-qt"
$BuildSun = Join-Path $Root "build\sunshine"
$BuildMoon = Join-Path $Root "build\moonlight"
$BinDir = Join-Path $Root "src-tauri\binaries"
$AssetDir = Join-Path $Root "src-tauri\resources\sunshine"
$Prefix = Join-Path $Root "src-tauri\resources\sunshine-prefix"

function Require-Cmd($name) {
    if (-not (Get-Command $name -ErrorAction SilentlyContinue)) {
        throw "Missing required tool: $name. Run scripts/bootstrap.ps1"
    }
}

Require-Cmd rustc
Require-Cmd cmake
Require-Cmd git
$qmake = Get-Command qmake -ErrorAction SilentlyContinue
if (-not $qmake) { $qmake = Get-Command qmake6 -ErrorAction SilentlyContinue }
if (-not $qmake) { throw "Missing qmake/qmake6 (Qt 6). Run scripts/bootstrap.ps1" }

if (-not (Test-Path (Join-Path $SunshineSrc "CMakeLists.txt"))) {
    git -C $Root submodule update --init --recursive
}
if (-not (Test-Path (Join-Path $SunshineSrc "CMakeLists.txt"))) {
    throw "Vendored engines missing. git submodule update --init --recursive (do not checkout Moonlight v6.1.0)"
}

$Triple = (rustc --print host-tuple).Trim()
Write-Host "Host triple: $Triple"

$cudaMode = $env:ECLIPSE_CUDA
if (-not $cudaMode) { $cudaMode = "auto" }
$useCuda = "OFF"
if ($cudaMode -eq "on") { $useCuda = "ON" }
elseif ($cudaMode -eq "auto" -and (Get-Command nvcc -ErrorAction SilentlyContinue)) { $useCuda = "ON" }
Write-Host "Sunshine CUDA: $useCuda"

if (-not (Test-EclipseMsys2SunshineDeps)) {
    throw "MSYS2 UCRT64 OpenSSL/gcc missing. Run scripts/bootstrap.ps1 (Sunshine cannot be built with MSVC)."
}

$cache = Join-Path $BuildSun "CMakeCache.txt"
if (Test-Path $cache) {
    $cacheText = Get-Content $cache -Raw -ErrorAction SilentlyContinue
    if ($cacheText -match "MSVC" -or $cacheText -match "Visual Studio") {
        Write-Host "Removing leftover MSVC Sunshine build dir (need MSYS2 UCRT64)"
        Remove-Item -Recurse -Force $BuildSun
    }
}

New-Item -ItemType Directory -Force -Path $BinDir, $AssetDir, $BuildSun, $BuildMoon, $Prefix | Out-Null

$sunTag = (git -C $SunshineSrc describe --tags --always).Trim()
$env:BRANCH = "master"
$env:BUILD_VERSION = $sunTag

$srcUnix = ConvertTo-EclipseMsysPath $SunshineSrc
$buildUnix = ConvertTo-EclipseMsysPath $BuildSun
$prefixUnix = ConvertTo-EclipseMsysPath $Prefix

Write-Host "==> Configuring Sunshine (MSYS2 UCRT64)"
Write-Host "Sunshine version: $sunTag"
# Single-line cmake: bash -c only reliably sees one -c argument, and CRLF
# backslash continuations are not line joins.
Invoke-EclipseMsys2 -Command "unset CMAKE_C_COMPILER CMAKE_CXX_COMPILER CC CXX CMAKE_GENERATOR; cmake -S '$srcUnix' -B '$buildUnix' -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX='$prefixUnix' -DSUNSHINE_ASSETS_DIR=share/sunshine -DBUILD_DOCS=OFF -DBUILD_TESTS=OFF -DBUILD_WERROR=OFF -DSUNSHINE_ENABLE_TRAY=OFF -DSUNSHINE_ENABLE_CUDA=$useCuda -DCUDA_FAIL_ON_MISSING=OFF -DSUNSHINE_PUBLISHER_NAME=Eclipse -DSUNSHINE_PUBLISHER_ISSUE_URL=https://github.com"

Write-Host "==> Building Sunshine"
Invoke-EclipseMsys2 -Command "cmake --build '$buildUnix' && cmake --install '$buildUnix'"

$sunBin = Get-ChildItem -Path $Prefix -Recurse -Filter "sunshine.exe" | Select-Object -First 1
if (-not $sunBin) { throw "sunshine.exe not found under $Prefix" }
Copy-Item $sunBin.FullName (Join-Path $BinDir "sunshine-$Triple.exe") -Force

$share = Join-Path $Prefix "share\sunshine"
if (Test-Path $share) {
    Copy-Item "$share\*" $AssetDir -Recurse -Force
}

$moonLibs = Join-Path $MoonlightSrc "libs\windows"
if (-not (Test-Path $moonLibs)) {
    $setupDeps = Join-Path $MoonlightSrc "setup-deps.ps1"
    if (-not (Test-Path $setupDeps)) {
        throw "Moonlight Windows deps missing and setup-deps.ps1 was not found at $setupDeps"
    }
    Write-Host "==> Fetching Moonlight Windows prebuilts"
    & $setupDeps
}

Write-Host "==> Building Moonlight"
Push-Location $BuildMoon
& $qmake.Source (Join-Path $MoonlightSrc "moonlight-qt.pro") "CONFIG+=release"
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "qmake failed (exit $LASTEXITCODE)"
}
if (Get-Command jom -ErrorAction SilentlyContinue) {
    jom
} elseif (Get-Command nmake -ErrorAction SilentlyContinue) {
    nmake release
} else {
    throw "Need nmake (VS Developer Prompt) or jom on PATH"
}
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "Moonlight nmake/jom failed (exit $LASTEXITCODE)"
}
Pop-Location

$moonBin = Get-ChildItem -Path $BuildMoon, $MoonlightSrc -Recurse -Filter "Moonlight.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $moonBin) {
    $moonBin = Get-ChildItem -Path $BuildMoon, $MoonlightSrc -Recurse -Filter "moonlight.exe" -ErrorAction SilentlyContinue |
        Select-Object -First 1
}
if (-not $moonBin) { throw "Moonlight.exe not found" }
Copy-Item $moonBin.FullName (Join-Path $BinDir "moonlight-$Triple.exe") -Force

Write-Host "Sidecars copied to $BinDir"
Get-ChildItem $BinDir

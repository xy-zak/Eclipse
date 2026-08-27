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

function Deploy-MoonlightRuntime {
    param([Parameter(Mandatory = $true)][string]$MoonlightExe)

    $deploy = Join-Path $Root "src-tauri\resources\moonlight-runtime"
    New-Item -ItemType Directory -Force -Path $deploy | Out-Null
    Get-ChildItem $deploy -Force | Where-Object { $_.Name -ne ".gitkeep" } | Remove-Item -Recurse -Force

    $libDir = Join-Path $MoonlightSrc "libs\windows\lib\x64"
    if (-not (Test-Path (Join-Path $libDir "SDL2.dll"))) {
        throw "Moonlight x64 DLLs missing at $libDir"
    }
    Copy-Item (Join-Path $libDir "*.dll") $deploy -Force

    $anti = Join-Path $BuildMoon "AntiHooking\release\AntiHooking.dll"
    if (-not (Test-Path $anti)) {
        $anti = Get-ChildItem -Path $BuildMoon -Recurse -Filter "AntiHooking.dll" |
            Select-Object -First 1 -ExpandProperty FullName
    }
    if (-not $anti -or -not (Test-Path $anti)) {
        throw "AntiHooking.dll not found under $BuildMoon"
    }
    Copy-Item $anti $deploy -Force

    $gameDb = Join-Path $MoonlightSrc "app\SDL_GameControllerDB\gamecontrollerdb.txt"
    if (Test-Path $gameDb) {
        Copy-Item $gameDb $deploy -Force
    }

    $qtBin = Split-Path $qmake.Source
    $windeployqt = Join-Path $qtBin "windeployqt.exe"
    if (-not (Test-Path $windeployqt)) {
        throw "windeployqt.exe missing next to qmake at $qtBin"
    }

    $qmlDir = Join-Path $MoonlightSrc "app\gui"
    & $windeployqt --dir $deploy --release --qmldir $qmlDir --no-opengl-sw --no-compiler-runtime --no-sql --no-system-d3d-compiler --no-system-dxc-compiler --skip-plugin-types qmltooling,generic --no-ffmpeg --no-quickcontrols2fusion --no-quickcontrols2imagine --no-quickcontrols2universal $MoonlightExe
    if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
        Write-Host "windeployqt extra flags failed; retrying with a smaller flag set"
        & $windeployqt --dir $deploy --release --qmldir $qmlDir --no-opengl-sw --no-compiler-runtime --no-sql $MoonlightExe
        if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
            throw "windeployqt failed (exit $LASTEXITCODE)"
        }
    }

    $icu = Join-Path $deploy "icuuc.dll"
    if (Test-Path $icu) {
        Remove-Item $icu -Force
    }
}

Require-Cmd rustc
Require-Cmd cmake
Require-Cmd git
$qmake = Get-Command qmake -ErrorAction SilentlyContinue
if (-not $qmake) { $qmake = Get-Command qmake6 -ErrorAction SilentlyContinue }
if (-not $qmake) { throw "Missing qmake/qmake6 (Qt 6). Run scripts/bootstrap.ps1" }

if ($env:ECLIPSE_SIDECAR -eq "moonlight-runtime") {
    $moonBin = Get-ChildItem -Path $BuildMoon, $MoonlightSrc -Recurse -Filter "Moonlight.exe" -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $moonBin) {
        throw "Moonlight.exe not found. Build Moonlight first."
    }
    Write-Host "==> Deploying Moonlight Windows runtime (Qt/SDL/FFmpeg)"
    Deploy-MoonlightRuntime -MoonlightExe $moonBin.FullName
    Write-Host "Moonlight runtime staged at src-tauri/resources/moonlight-runtime"
    exit 0
}

if (-not (Test-Path (Join-Path $SunshineSrc "CMakeLists.txt"))) {
    git -C $Root submodule update --init --recursive
}
if (-not (Test-Path (Join-Path $SunshineSrc "CMakeLists.txt"))) {
    throw "Vendored engines missing. git submodule update --init --recursive (do not checkout Moonlight v6.1.0)"
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)][string]$Repo,
        [Parameter(Mandatory = $true)][string[]]$GitArgs,
        [switch]$Quiet
    )
    # git writes "error:" / "warning:" to stderr even for expected --check misses.
    # With $ErrorActionPreference Stop that becomes a terminating NativeCommandError.
    # Capture streams so stdout does not become this function's return value.
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        if ($Quiet) {
            & git -C $Repo @GitArgs *> $null
        } else {
            $output = & git -C $Repo @GitArgs 2>&1
            foreach ($line in @($output)) {
                Write-Host "$line"
            }
        }
        return [int]$LASTEXITCODE
    } finally {
        $ErrorActionPreference = $prev
    }
}

function Get-VendorPinSha {
    param([Parameter(Mandatory = $true)][string]$VendorRelPath)
    $pins = Join-Path $Root "vendor\PINS"
    foreach ($line in Get-Content $pins) {
        if ($line -match '^\s*#' -or $line -match '^\s*$') { continue }
        $parts = $line -split '\s+'
        if ($parts.Count -ge 2 -and $parts[0] -eq $VendorRelPath) {
            return $parts[1]
        }
    }
    throw "No pin for $VendorRelPath in vendor/PINS"
}

function Reset-VendorToPin {
    param(
        [Parameter(Mandatory = $true)][string]$Repo,
        [Parameter(Mandatory = $true)][string]$VendorRelPath
    )
    $sha = Get-VendorPinSha $VendorRelPath
    Write-Host "==> Resetting $VendorRelPath to pin $sha"
    $reset = Invoke-Git -Repo $Repo -GitArgs @("reset", "--hard", $sha)
    if ($reset -ne 0) {
        throw "git reset --hard $sha failed in $Repo (exit $reset)"
    }
    # -fd keeps gitignored prebuilts (e.g. moonlight libs/windows).
    $clean = Invoke-Git -Repo $Repo -GitArgs @("clean", "-fd")
    if ($clean -ne 0) {
        throw "git clean -fd failed in $Repo (exit $clean)"
    }
}

function Apply-SidecarPatches {
    param(
        [Parameter(Mandatory = $true)][string]$Repo,
        [Parameter(Mandatory = $true)][string]$Filter
    )
    $patchDir = Join-Path $Root "patches"
    $patches = @(Get-ChildItem $patchDir -Filter $Filter -ErrorAction SilentlyContinue | Sort-Object Name)
    if ($patches.Count -eq 0) {
        throw "No patches/$Filter files found"
    }
    foreach ($patch in $patches) {
        Write-Host "==> Applying $($patch.Name)"
        $applied = Invoke-Git -Repo $Repo -GitArgs @("apply", "--whitespace=nowarn", $patch.FullName)
        if ($applied -ne 0) {
            throw "git apply failed for $($patch.Name) (exit $applied). Vendor tree must be the clean pin."
        }
    }
}

function Apply-SunshinePatches {
    Reset-VendorToPin -Repo $SunshineSrc -VendorRelPath "vendor/sunshine"
    Apply-SidecarPatches -Repo $SunshineSrc -Filter "sunshine-*.patch"
}

function Apply-MoonlightPatches {
    Reset-VendorToPin -Repo $MoonlightSrc -VendorRelPath "vendor/moonlight-qt"
    Apply-SidecarPatches -Repo $MoonlightSrc -Filter "moonlight-*.patch"
    # Guard against a silent partial UI apply (Material colors without dialog chrome).
    $dialog = Join-Path $MoonlightSrc "app\gui\NavigableMessageDialog.qml"
    $mainCpp = Join-Path $MoonlightSrc "app\main.cpp"
    $dialogText = Get-Content $dialog -Raw
    $mainText = Get-Content $mainCpp -Raw
    if ($dialogText -notmatch 'exclamation\.svg' -or $dialogText -notmatch 'frameColor') {
        throw "moonlight-02-ui.patch incomplete: NavigableMessageDialog.qml missing Eclipse dialog chrome"
    }
    if ($mainText -notmatch 'JetBrains Mono') {
        throw "moonlight-01-behavior.patch incomplete: main.cpp missing JetBrains Mono app font"
    }
}

# Sunshine is a CUI binary. A GUI Eclipse host must not flash a console when
# the service or sidecar launches it. Mark the PE subsystem as WINDOWS
# (IMAGE_SUBSYSTEM_WINDOWS_GUI). Official Sunshine's own installer does this
# too; we are not changing the SCM service name (must stay SunshineService).
function Set-SunshineWindowsGui {
    param([Parameter(Mandatory = $true)][string]$Exe)
    $bytes = [System.IO.File]::ReadAllBytes($Exe)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
        throw "Not a PE image: $Exe"
    }
    $eLfanew = [BitConverter]::ToInt32($bytes, 0x3C)
    $magicOff = $eLfanew + 24
    $magic = [BitConverter]::ToUInt16($bytes, $magicOff)
    if ($magic -ne 0x20B) {
        throw "Expected PE32+ optional header in $Exe (magic=$magic)"
    }
    $subsysOff = $magicOff + 68
    $bytes[$subsysOff] = 2
    $bytes[$subsysOff + 1] = 0
    [System.IO.File]::WriteAllBytes($Exe, $bytes)
    Write-Host "Set WINDOWS subsystem on $Exe"
}

Apply-SunshinePatches
Apply-MoonlightPatches

$Triple = (rustc --print host-tuple).Trim()
Write-Host "Host triple: $Triple"

# SUNSHINE_ENABLE_CUDA is Linux-only (options.cmake). Windows uses NVENC via
# the nvenc_* targets; passing -DSUNSHINE_ENABLE_CUDA triggers unused-cli.
Write-Host "Sunshine CUDA: N/A on Windows (NVENC); ECLIPSE_CUDA does not affect this CMake"

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

$sunIcoUnix = ""
$sunIco = Join-Path $Root "src-tauri\icons\sunshine.ico"
if (Test-Path $sunIco) {
    $sunIcoUnix = ConvertTo-EclipseMsysPath $sunIco
}

Write-Host "==> Configuring Sunshine (MSYS2 UCRT64)"
Write-Host "Sunshine version: $sunTag"
# Single-line cmake: bash -c only reliably sees one -c argument, and CRLF
# backslash continuations are not line joins.
$icoFlag = ""
if ($sunIcoUnix) { $icoFlag = "-DSUNSHINE_ICON_PATH='$sunIcoUnix'" }
Invoke-EclipseMsys2 -Command "unset CMAKE_C_COMPILER CMAKE_CXX_COMPILER CC CXX CMAKE_GENERATOR; cmake -S '$srcUnix' -B '$buildUnix' -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX='$prefixUnix' -DSUNSHINE_ASSETS_DIR=share/sunshine -DSUNSHINE_ASSETS_DIR_DEF=share/sunshine -DBUILD_DOCS=OFF -DBUILD_TESTS=OFF -DBUILD_WERROR=OFF -DSUNSHINE_ENABLE_TRAY=OFF $icoFlag -DCUDA_FAIL_ON_MISSING=OFF -DSUNSHINE_PUBLISHER_NAME=Eclipse -DSUNSHINE_PUBLISHER_ISSUE_URL=https://github.com"

Write-Host "==> Building Sunshine"
Invoke-EclipseMsys2 -Command "cmake --build '$buildUnix' && cmake --install '$buildUnix'"

$sunBin = Get-ChildItem -Path $Prefix -Recurse -Filter "sunshine.exe" | Select-Object -First 1
if (-not $sunBin) { throw "sunshine.exe not found under $Prefix" }
Copy-Item $sunBin.FullName (Join-Path $BinDir "sunshine-$Triple.exe") -Force
Set-SunshineWindowsGui (Join-Path $BinDir "sunshine-$Triple.exe")

$svcBin = Get-ChildItem -Path $BuildSun, $Prefix -Recurse -Filter "sunshinesvc.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $svcBin) { throw "sunshinesvc.exe not found under $BuildSun or $Prefix" }
$svcDest = Join-Path $Root "src-tauri\resources\windows"
New-Item -ItemType Directory -Force -Path $svcDest | Out-Null
Copy-Item $svcBin.FullName (Join-Path $svcDest "sunshinesvc.exe") -Force
Write-Host "Staged sunshinesvc.exe at $svcDest"

$share = Join-Path $Prefix "share\sunshine"
if (Test-Path $share) {
    Copy-Item "$share\*" $AssetDir -Recurse -Force
}
$sunSvg = Join-Path $Root "src-tauri\icons\sunshine.svg"
$webImages = Join-Path $AssetDir "web\images"
if ((Test-Path $sunSvg) -and (Test-Path $webImages)) {
    Copy-Item $sunSvg (Join-Path $webImages "logo-sunshine.svg") -Force
    if (Test-Path $sunIco) {
        Copy-Item $sunIco (Join-Path $webImages "sunshine.ico") -Force
    }
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
$fontDir = Join-Path $Root "assets\fonts"
$moonFontDir = Join-Path $MoonlightSrc "app\res\eclipse"
New-Item -ItemType Directory -Force -Path $moonFontDir | Out-Null
foreach ($font in @("Remboy-Regular.ttf", "JetBrainsMono-Regular.ttf")) {
    $src = Join-Path $fontDir $font
    if (-not (Test-Path $src)) { throw "Missing Eclipse font $src" }
    Copy-Item $src $moonFontDir -Force
}
$gifSrc = Join-Path $Root "assets\eclipse.gif"
if (-not (Test-Path $gifSrc)) { throw "Missing Eclipse splash $gifSrc" }
Copy-Item $gifSrc $moonFontDir -Force
$moonIco = Join-Path $Root "src-tauri\icons\moonlight.ico"
if (Test-Path $moonIco) {
    Copy-Item $moonIco (Join-Path $MoonlightSrc "app\moonlight.ico") -Force
}
Push-Location $BuildMoon
& $qmake.Source (Join-Path $MoonlightSrc "moonlight-qt.pro") "CONFIG+=release"
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "qmake failed (exit $LASTEXITCODE)"
}
Invoke-EclipseNmake -WorkingDirectory $BuildMoon -ArgumentList @("release")
Pop-Location

$moonBin = Get-ChildItem -Path $BuildMoon, $MoonlightSrc -Recurse -Filter "Moonlight.exe" -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $moonBin) {
    $moonBin = Get-ChildItem -Path $BuildMoon, $MoonlightSrc -Recurse -Filter "moonlight.exe" -ErrorAction SilentlyContinue |
        Select-Object -First 1
}
if (-not $moonBin) { throw "Moonlight.exe not found" }
Copy-Item $moonBin.FullName (Join-Path $BinDir "moonlight-$Triple.exe") -Force

Write-Host "==> Deploying Moonlight Windows runtime (Qt/SDL/FFmpeg)"
Deploy-MoonlightRuntime -MoonlightExe $moonBin.FullName

Write-Host "Sidecars copied to $BinDir"
Get-ChildItem $BinDir

# Install the Windows 11 toolchain for Eclipse sidecar + Tauri builds.
# Self-elevates once via UAC. Idempotent.
# CUDA toolkit is installed only when an NVIDIA GPU is present unless ECLIPSE_CUDA=off.
# Sunshine is compiled with MSYS2 UCRT64 (OpenSSL and friends). Moonlight uses MSVC + Qt.
# Does not install the NVIDIA Game Ready driver.
param(
    [switch]$Elevated
)

$ErrorActionPreference = "Stop"

$QtVersion = "6.8.3"
$QtArch = "win64_msvc2022_64"
$QtRoot = Join-Path $env:LOCALAPPDATA "Eclipse\Qt"
$EclipseLog = Join-Path $env:TEMP "eclipse-windows-setup.log"

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

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Test-Cmd($name) {
    return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

function Update-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machine;$user"
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargo) { $env:Path = "$cargo;$env:Path" }
}

function Add-UserPathEntry([string]$Dir) {
    if (-not $Dir -or -not (Test-Path $Dir)) { return }
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    if (-not $user) { $user = "" }
    $parts = @($user -split ";" | Where-Object { $_ -ne "" })
    if ($parts -contains $Dir) { return }
    $new = if ($user) { "$Dir;$user" } else { $Dir }
    [Environment]::SetEnvironmentVariable("Path", $new, "User")
    Update-SessionPath
}

function Install-WingetPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [string]$Override = ""
    )
    Write-Host "  winget install $Id"
    $wingetArgs = @(
        "install", "--id", $Id, "-e", "--source", "winget",
        "--accept-package-agreements", "--accept-source-agreements"
    )
    if ($Override) {
        $wingetArgs += @("--override", $Override)
    }
    & winget @wingetArgs
    $code = $LASTEXITCODE
    # 0 = ok; -1978335189 already installed; -1978335135 no applicable upgrade
    if ($code -ne 0 -and $code -ne -1978335189 -and $code -ne -1978335135) {
        throw "winget install failed for $Id (exit $code)"
    }
    Update-SessionPath
}

function Test-Msvc {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { return $false }
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    return [bool]$vs
}

function Test-NvidiaGpu {
    if (Test-Cmd "nvidia-smi") { return $true }
    try {
        $gpus = Get-CimInstance Win32_VideoController -ErrorAction Stop
        foreach ($g in $gpus) {
            if ($g.Name -match "NVIDIA") { return $true }
        }
    } catch {
        return $false
    }
    return $false
}

function Want-Cuda {
    $mode = $env:ECLIPSE_CUDA
    if (-not $mode) { $mode = "auto" }
    switch ($mode.ToLower()) {
        "off" { return $false }
        "on" { return $true }
        default { return (Test-NvidiaGpu) }
    }
}

function Find-Qmake {
    $q = Get-Command qmake -ErrorAction SilentlyContinue
    if (-not $q) { $q = Get-Command qmake6 -ErrorAction SilentlyContinue }
    if ($q) { return $q }
    $found = Get-ChildItem -Path $QtRoot -Recurse -Filter "qmake.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match "msvc2022_64\\bin\\qmake\.exe$" } |
        Select-Object -First 1
    if ($found) {
        Add-UserPathEntry $found.DirectoryName
        return Get-Command qmake -ErrorAction SilentlyContinue
    }
    return $null
}

function Install-QtMsvc {
    if (Find-Qmake) {
        Write-Host "  OK  qmake already present"
        return
    }
    if (-not (Test-Cmd "python")) {
        Install-WingetPackage -Id "Python.Python.3.12"
        Update-SessionPath
        $pyDirs = @(
            Join-Path $env:LOCALAPPDATA "Programs\Python\Python312"
            Join-Path $env:LOCALAPPDATA "Programs\Python\Python313"
            "C:\Program Files\Python312"
            "C:\Program Files\Python313"
        )
        foreach ($d in $pyDirs) {
            if (Test-Path (Join-Path $d "python.exe")) {
                Add-UserPathEntry $d
                Add-UserPathEntry (Join-Path $d "Scripts")
            }
        }
        Update-SessionPath
    }
    if (-not (Test-Cmd "python")) {
        throw "Python is required to install Qt via aqtinstall, but python is not on PATH."
    }
    Write-Host "  pip install aqtinstall"
    python -m pip install --user --upgrade aqtinstall
    Update-SessionPath
    Write-Host "  aqt install-qt $QtVersion $QtArch (several GB)..."
    New-Item -ItemType Directory -Force -Path $QtRoot | Out-Null
    # Qt 6.8 ships qtdeclarative/qtsvg in the base kit. Passing them as -m extras
    # makes aqt fail immediately ("packages were not found while parsing XML").
    $wanted = @("qtshadertools", "qtdeclarative", "qtsvg", "qtimageformats")
    $listed = python -m aqt list-qt windows desktop --modules $QtVersion $QtArch
    if ($LASTEXITCODE -ne 0) {
        throw "aqt list-qt failed for $QtVersion $QtArch"
    }
    $available = @($listed -split "\s+" | Where-Object { $_ })
    $mods = @($wanted | Where-Object { $available -contains $_ })
    $aqtArgs = @(
        "-m", "aqt", "install-qt", "windows", "desktop", $QtVersion, $QtArch,
        "--outputdir", $QtRoot
    )
    if ($mods.Count -gt 0) {
        Write-Host "  extra Qt modules: $($mods -join ', ')"
        $aqtArgs += @("-m") + $mods
    }
    python @aqtArgs
    if ($LASTEXITCODE -ne 0) {
        throw "aqt install-qt failed (exit $LASTEXITCODE). See $EclipseLog"
    }
    $qmakePath = Join-Path $QtRoot "$QtVersion\msvc2022_64\bin"
    if (-not (Test-Path (Join-Path $qmakePath "qmake.exe"))) {
        throw "Qt install finished but qmake.exe was not found at $qmakePath"
    }
    Add-UserPathEntry $qmakePath
}

function Install-CudaToolkit {
    if (-not (Want-Cuda)) {
        Write-Host "  CUDA  skipped (no NVIDIA GPU, or ECLIPSE_CUDA=off)"
        return
    }
    Update-SessionPath
    if (Test-Cmd "nvcc") {
        Write-Host "  CUDA  nvcc already present"
        return
    }
    if ((Test-NvidiaGpu) -and -not (Test-Cmd "nvidia-smi")) {
        Write-Host "  CUDA  NVIDIA GPU detected but nvidia-smi is missing. Install the Game Ready driver for runtime NVENC; installing the CUDA toolkit for compile."
    }
    Write-Host "  CUDA  installing toolkit (several GB)..."
    $ok = $false
    foreach ($id in @("Nvidia.CUDA", "NVIDIA.CUDA")) {
        try {
            Install-WingetPackage -Id $id
            $ok = $true
            break
        } catch {
            Write-Host "  CUDA  $id not available: $($_.Exception.Message)"
        }
    }
    if (-not $ok) {
        throw "Could not install CUDA toolkit via winget (tried Nvidia.CUDA / NVIDIA.CUDA)."
    }
    $cudaRoot = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA"
    if (Test-Path $cudaRoot) {
        $latest = Get-ChildItem -Path $cudaRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
        if ($latest) {
            Add-UserPathEntry (Join-Path $latest.FullName "bin")
        }
    }
    Update-SessionPath
}

function Install-Msys2SunshineDeps {
    . (Join-Path $PSScriptRoot "windows-dev-env.ps1")
    $root = Get-EclipseMsys2Root
    $bash = Join-Path $root "usr\bin\bash.exe"
    if (-not (Test-Path $bash)) {
        Write-Host "  Installing MSYS2 (Sunshine on Windows needs UCRT64, not MSVC)..."
        Install-WingetPackage -Id "MSYS2.MSYS2"
        Start-Sleep -Seconds 2
        $root = Get-EclipseMsys2Root
        $bash = Join-Path $root "usr\bin\bash.exe"
        if (-not (Test-Path $bash)) {
            throw "MSYS2 install finished but bash.exe was not found at $bash"
        }
    } else {
        Write-Host "  OK  MSYS2"
    }

    if (Test-EclipseMsys2SunshineDeps) {
        Write-Host "  OK  MSYS2 UCRT64 OpenSSL / gcc"
        return
    }

    Write-Host "  pacman -Syuu (MSYS2 core)..."
    for ($i = 0; $i -lt 3; $i++) {
        $code = Invoke-EclipseMsys2 -PassThru -Command "pacman -Syuu --noconfirm"
        if ($code -eq 0) { break }
        Write-Host "  pacman core update pass $($i + 1) exited $code (retrying is normal)"
    }

    # Do not install the "toolchain" group: pacman still prompts for member
    # selection even with --noconfirm. gcc pulls in crt/headers/binutils.
    # MinHook's package name is case-sensitive on the UCRT64 repo.
    $pkgs = @(
        "mingw-w64-ucrt-x86_64-gcc",
        "mingw-w64-ucrt-x86_64-cmake",
        "mingw-w64-ucrt-x86_64-ninja",
        "mingw-w64-ucrt-x86_64-pkgconf",
        "mingw-w64-ucrt-x86_64-openssl",
        "mingw-w64-ucrt-x86_64-curl-winssl",
        "mingw-w64-ucrt-x86_64-miniupnpc",
        "mingw-w64-ucrt-x86_64-opus",
        "mingw-w64-ucrt-x86_64-boost",
        "mingw-w64-ucrt-x86_64-nlohmann-json",
        "mingw-w64-ucrt-x86_64-onevpl",
        "mingw-w64-ucrt-x86_64-cppwinrt",
        "mingw-w64-ucrt-x86_64-MinHook"
    )
    Write-Host "  pacman -S Sunshine UCRT64 deps..."
    Invoke-EclipseMsys2 -Command ("pacman -S --needed --noconfirm " + ($pkgs -join " "))
    if (-not (Test-EclipseMsys2SunshineDeps)) {
        throw "MSYS2 UCRT64 OpenSSL/gcc still missing after pacman. See $EclipseLog"
    }
}

function Assert-Toolchain {
    . (Join-Path $PSScriptRoot "windows-dev-env.ps1")
    $ok = $true
    foreach ($cmd in @("git", "node", "npm", "rustc", "cargo", "cmake", "ninja")) {
        if (Test-Cmd $cmd) {
            Write-Host "  OK  $cmd"
        } else {
            Write-Host "  MISSING  $cmd"
            $ok = $false
        }
    }
    $qmake = Get-Command qmake -ErrorAction SilentlyContinue
    if (-not $qmake) { $qmake = Get-Command qmake6 -ErrorAction SilentlyContinue }
    if ($qmake) {
        Write-Host "  OK  $($qmake.Name)"
    } else {
        Write-Host "  MISSING  qmake / qmake6"
        $ok = $false
    }
    if (Test-Msvc) {
        Write-Host "  OK  MSVC"
    } else {
        Write-Host "  MISSING  Visual Studio C++ workload"
        $ok = $false
    }
    if (Test-Cmd "nmake") {
        Write-Host "  OK  nmake"
    } else {
        Write-Host "  MISSING  nmake (VsDevCmd)"
        $ok = $false
    }
    if (Test-Cmd "nvcc") {
        Write-Host "  CUDA  nvcc found (Sunshine CUDA will be ON unless ECLIPSE_CUDA=off)"
        nvcc --version | Select-Object -First 3
    } else {
        Write-Host "  CUDA  nvcc not found (Sunshine CUDA will be OFF unless ECLIPSE_CUDA=on)"
    }
    if (Test-EclipseMsys2SunshineDeps) {
        Write-Host "  OK  MSYS2 UCRT64 OpenSSL / gcc"
    } else {
        Write-Host "  MISSING  MSYS2 UCRT64 OpenSSL / gcc"
        $ok = $false
    }
    if (-not $ok) {
        throw "Bootstrap finished but required tools are still missing. Open a new PowerShell and re-run."
    }
}

if (-not (Test-Cmd "winget")) {
    throw "winget is required (Windows 11 App Installer). Install it from the Microsoft Store, then re-run."
}

if (-not (Test-Admin)) {
    Write-Host "Eclipse Windows bootstrap needs Administrator once (UAC) to install VS Build Tools / CUDA / packages."
    $cwd = $PWD.Path -replace "'", "''"
    $self = $PSCommandPath -replace "'", "''"
    $log = $EclipseLog -replace "'", "''"
    $cuda = [string]$env:ECLIPSE_CUDA
    $pause = [string]$env:ECLIPSE_NO_PAUSE
    $p = Start-Process -FilePath "powershell.exe" -Verb RunAs -Wait -PassThru -ArgumentList @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-Command",
        @"
`$ErrorActionPreference = 'Stop'
`$env:ECLIPSE_CUDA = '$cuda'
`$env:ECLIPSE_NO_PAUSE = '$pause'
try {
    Start-Transcript -Path '$log' -Force | Out-Null
    Set-Location '$cwd'
    & '$self' -Elevated
    `$code = [int]`$LASTEXITCODE
    Stop-Transcript | Out-Null
    exit `$code
} catch {
    try { Stop-Transcript | Out-Null } catch {}
    Write-Host `$_ -ForegroundColor Red
    Write-Host "Log: $log"
    if (-not `$env:ECLIPSE_NO_PAUSE) {
        Read-Host 'Press Enter to close this window' | Out-Null
    }
    exit 1
}
"@
    )
    if ($p.ExitCode -ne 0) {
        Write-Host ""
        Write-Host "Elevated bootstrap failed (exit $($p.ExitCode))." -ForegroundColor Red
        if (Test-Path $EclipseLog) {
            Write-Host "----- $EclipseLog -----"
            Get-Content $EclipseLog | Write-Host
            Write-Host "----- end log -----"
        }
        Show-EclipseFailure "Fix the error above, then re-run scripts/setup-windows.ps1 from PowerShell (not by double-clicking)."
        exit $p.ExitCode
    }
    Write-Host "Installers finished. Refreshing this session..."
    try {
        Assert-Toolchain
    } catch {
        Show-EclipseFailure $_.Exception.Message
        exit 1
    }
    Write-Host "Bootstrap finished."
    exit 0
}

Write-Host "Eclipse Windows bootstrap (installs missing tools)"
Write-Host "VS Build Tools, Qt, CUDA, and MSYS2 UCRT64 (Sunshine) are large downloads. ECLIPSE_CUDA=off skips CUDA."
Write-Host "This script does not install the NVIDIA Game Ready driver."

Update-SessionPath

if (-not (Test-Cmd "git")) { Install-WingetPackage -Id "Git.Git" } else { Write-Host "  OK  git" }
if (-not (Test-Cmd "node")) { Install-WingetPackage -Id "OpenJS.NodeJS.LTS" } else { Write-Host "  OK  node" }
if (-not (Test-Cmd "cmake")) { Install-WingetPackage -Id "Kitware.CMake" } else { Write-Host "  OK  cmake" }
if (-not (Test-Cmd "ninja")) { Install-WingetPackage -Id "Ninja-build.Ninja" } else { Write-Host "  OK  ninja" }

if (-not (Test-Cmd "rustup") -and -not (Test-Cmd "rustc")) {
    Install-WingetPackage -Id "Rustlang.Rustup"
}
Update-SessionPath
$cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $cargoBin) { Add-UserPathEntry $cargoBin }
if (Test-Cmd "rustup") {
    rustup default stable
} elseif (-not (Test-Cmd "rustc")) {
    throw "rustup/rustc missing after install."
}

if (-not (Test-Msvc)) {
    Write-Host "  Installing Visual Studio 2022 Build Tools (C++ workload)..."
    Install-WingetPackage -Id "Microsoft.VisualStudio.2022.BuildTools" -Override "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
} else {
    Write-Host "  OK  MSVC"
}

Install-QtMsvc
Install-CudaToolkit
Install-Msys2SunshineDeps

if ($Elevated) {
    # Parent non-admin process will verify after PATH refresh.
    Write-Host "Elevated installs complete."
    exit 0
}

try {
    Assert-Toolchain
    if (Test-Cmd "rustup") { rustup default stable | Out-Host }
    Write-Host "Bootstrap finished."
} catch {
    Show-EclipseFailure $_.Exception.Message
    exit 1
}

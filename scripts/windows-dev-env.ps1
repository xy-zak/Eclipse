# Shared Windows session environment for Eclipse bootstrap and sidecar builds.
# Dot-source this script. Refreshes PATH, Qt, CUDA, cargo, VsDevCmd (nmake/cl),
# and helpers to run Sunshine builds inside MSYS2 UCRT64.

function Add-EclipseToolPaths {
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path $cargo) {
        $env:Path = "$cargo;$env:Path"
    }

    $localBin = Join-Path $env:USERPROFILE ".local\bin"
    if (Test-Path $localBin) {
        $env:Path = "$localBin;$env:Path"
    }

    $qtRoot = Join-Path $env:LOCALAPPDATA "Eclipse\Qt"
    if (Test-Path $qtRoot) {
        $qmake = Get-ChildItem -Path $qtRoot -Recurse -Filter "qmake.exe" -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match "msvc2022_64\\bin\\qmake\.exe$" } |
            Select-Object -First 1
        if ($qmake) {
            $env:Path = "$($qmake.DirectoryName);$env:Path"
        }
    }

    $cudaRoot = "C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA"
    if (Test-Path $cudaRoot) {
        $latest = Get-ChildItem -Path $cudaRoot -Directory -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending |
            Select-Object -First 1
        if ($latest) {
            $cudaBin = Join-Path $latest.FullName "bin"
            if (Test-Path $cudaBin) {
                $env:Path = "$cudaBin;$env:Path"
            }
        }
    }
}

function Update-EclipseSessionPath {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $pieces = @()
    if ($machine) { $pieces += $machine }
    if ($user) { $pieces += $user }
    $env:Path = ($pieces -join ";")
    Add-EclipseToolPaths
}

function Import-EclipseVsDevCmd {
    if (Get-Command nmake -ErrorAction SilentlyContinue) {
        return
    }
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) {
        return
    }
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vs) {
        return
    }
    $vsdev = Join-Path $vs.Trim() "Common7\Tools\VsDevCmd.bat"
    if (-not (Test-Path $vsdev)) {
        return
    }
    $envDump = cmd.exe /c "`"$vsdev`" -arch=x64 -host_arch=x64 -no_logo && set"
    foreach ($line in ($envDump -split "`r?`n")) {
        if ($line -match "^([^=]+)=(.*)$") {
            $name = $Matches[1]
            $value = $Matches[2]
            if ($name -eq "Path" -or $name -eq "PATH") {
                $env:Path = $value
            } else {
                Set-Item -Path "Env:$name" -Value $value
            }
        }
    }
}

function Get-EclipseMsys2Root {
    foreach ($candidate in @("C:\msys64", "C:\tools\msys64")) {
        if (Test-Path (Join-Path $candidate "usr\bin\bash.exe")) {
            return $candidate
        }
    }
    return "C:\msys64"
}

function ConvertTo-EclipseMsysPath {
    param([Parameter(Mandatory = $true)][string]$WinPath)
    $full = [System.IO.Path]::GetFullPath($WinPath)
    $drive = $full.Substring(0, 1).ToLowerInvariant()
    $rest = $full.Substring(2).Replace("\", "/")
    return "/${drive}${rest}"
}

function Test-EclipseMsys2SunshineDeps {
    $root = Get-EclipseMsys2Root
    return (Test-Path (Join-Path $root "ucrt64\include\openssl\ssl.h")) -and
        (Test-Path (Join-Path $root "ucrt64\bin\gcc.exe"))
}

function Invoke-EclipseMsys2 {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [switch]$PassThru
    )
    $bash = Join-Path (Get-EclipseMsys2Root) "usr\bin\bash.exe"
    if (-not (Test-Path $bash)) {
        throw "MSYS2 bash not found at $bash. Run scripts/bootstrap.ps1"
    }
    $prev = @{
        MSYSTEM          = $env:MSYSTEM
        CHERE_INVOKING   = $env:CHERE_INVOKING
        MSYS2_PATH_TYPE  = $env:MSYS2_PATH_TYPE
        CC               = $env:CC
        CXX              = $env:CXX
        CMAKE_GENERATOR  = $env:CMAKE_GENERATOR
        CMAKE_C_COMPILER = $env:CMAKE_C_COMPILER
        CMAKE_CXX_COMPILER = $env:CMAKE_CXX_COMPILER
    }
    $env:MSYSTEM = "UCRT64"
    $env:CHERE_INVOKING = "1"
    $env:MSYS2_PATH_TYPE = "inherit"
    $env:CC = $null
    $env:CXX = $null
    $env:CMAKE_GENERATOR = $null
    $env:CMAKE_C_COMPILER = $null
    $env:CMAKE_CXX_COMPILER = $null
    $nativePref = $null
    if (Test-Path variable:/PSNativeCommandUseErrorActionPreference) {
        $nativePref = $PSNativeCommandUseErrorActionPreference
        $PSNativeCommandUseErrorActionPreference = $false
    }
    $code = 1
    $oldEap = $ErrorActionPreference
    try {
        # CMake MESSAGE() and pacman write to stderr. 2>&1 + Stop turns that
        # into a terminating error and kills configure mid-run.
        $ErrorActionPreference = "Continue"
        & $bash --login -c $Command
        $code = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $oldEap
        if ($null -ne $nativePref) {
            $PSNativeCommandUseErrorActionPreference = $nativePref
        }
        foreach ($k in $prev.Keys) {
            if ($null -eq $prev[$k] -or $prev[$k] -eq "") {
                Remove-Item -Path "Env:$k" -ErrorAction SilentlyContinue
            } else {
                Set-Item -Path "Env:$k" -Value $prev[$k]
            }
        }
    }
    if ($PassThru) {
        return $code
    }
    if ($code -ne 0) {
        throw "MSYS2 UCRT64 command failed (exit $code)"
    }
}

Update-EclipseSessionPath
Import-EclipseVsDevCmd
# Re-prepend Qt/CUDA/cargo without resetting PATH — a second
# Update-EclipseSessionPath would drop nmake/cl from VsDevCmd.
Add-EclipseToolPaths

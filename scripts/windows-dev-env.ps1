# Shared Windows session environment for Eclipse bootstrap and sidecar builds.
# Dot-source this script. Refreshes PATH, Qt, CUDA, cargo, and VsDevCmd (nmake/cl).

function Update-EclipseSessionPath {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $pieces = @()
    if ($machine) { $pieces += $machine }
    if ($user) { $pieces += $user }
    $env:Path = ($pieces -join ";")

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
    foreach ($line in $envDump) {
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

Update-EclipseSessionPath
Import-EclipseVsDevCmd
Update-EclipseSessionPath

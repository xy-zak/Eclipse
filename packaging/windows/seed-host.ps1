# Merge Sunshine conf, seed random Web UI creds. Invoked from NSIS after files are copied.
param(
    [Parameter(Mandatory = $true)]
    [string]$InstallDir
)

$ErrorActionPreference = "Stop"
$ConfigDir = Join-Path $InstallDir "config"
$ConfPath = Join-Path $ConfigDir "sunshine.conf"
$Template = Join-Path $InstallDir "sunshine.conf"
$StatePath = Join-Path $ConfigDir "sunshine_state.json"
$PassFile = Join-Path $ConfigDir "web-ui-password"
$Sunshine = Join-Path $InstallDir "sunshine.exe"

function Get-JsonString([string]$Path, [string]$Key) {
    if (-not (Test-Path $Path)) { return $null }
    $text = Get-Content -Raw -Path $Path -ErrorAction SilentlyContinue
    if (-not $text) { return $null }
    $m = [regex]::Match($text, '"' + [regex]::Escape($Key) + '"\s*:\s*"([^"]*)"')
    if ($m.Success) { return $m.Groups[1].Value }
    return $null
}

function Get-SunshinePasswordHex([string]$Password, [string]$Salt) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($Password + $Salt)
        $digest = $sha.ComputeHash($bytes)
        [array]::Reverse($digest)
        return ([BitConverter]::ToString($digest) -replace '-', '').ToUpperInvariant()
    } finally {
        $sha.Dispose()
    }
}

function Test-CredsPresent {
    $u = Get-JsonString $StatePath "username"
    $p = Get-JsonString $StatePath "password"
    $s = Get-JsonString $StatePath "salt"
    return ($u -and $p -and $s)
}

function Test-LegacyEclipseCreds {
    $u = Get-JsonString $StatePath "username"
    $p = Get-JsonString $StatePath "password"
    $s = Get-JsonString $StatePath "salt"
    if ($u -ne "eclipse" -or -not $p -or -not $s) { return $false }
    $expect = Get-SunshinePasswordHex "eclipse" $s
    return ($p.ToUpperInvariant() -eq $expect)
}

function Merge-SunshineConf {
    New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
    if (-not (Test-Path $ConfPath)) {
        if (Test-Path $Template) {
            Copy-Item $Template $ConfPath
        } else {
            @"
lan_encryption_mode = 2
wan_encryption_mode = 2
origin_web_ui_allowed = lan
upnp = disabled
system_tray = disabled
"@ | Set-Content -Path $ConfPath -Encoding utf8
        }
    }
    $keys = @{
        "lan_encryption_mode" = "2"
        "wan_encryption_mode" = "2"
        "origin_web_ui_allowed" = "lan"
        "upnp" = "disabled"
        "system_tray" = "disabled"
    }
    $lines = @(Get-Content $ConfPath)
    foreach ($key in $keys.Keys) {
        $val = $keys[$key]
        $replacement = "$key = $val"
        $found = $false
        for ($i = 0; $i -lt $lines.Count; $i++) {
            $t = $lines[$i].TrimStart()
            if ($t.StartsWith("#") -or -not $t.Contains("=")) { continue }
            $k = $t.Split("=", 2)[0].Trim()
            if ($k -eq $key) {
                $lines[$i] = $replacement
                $found = $true
                break
            }
        }
        if (-not $found) {
            $lines += $replacement
        }
    }
    Set-Content -Path $ConfPath -Value $lines -Encoding utf8
}

function New-UiPassword {
    $chars = 'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789'
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try {
        $bytes = New-Object byte[] 20
        $rng.GetBytes($bytes)
        return -join ($bytes | ForEach-Object { $chars[$_ % $chars.Length] })
    } finally {
        $rng.Dispose()
    }
}

Merge-SunshineConf

$needCreds = -not (Test-CredsPresent) -or (Test-LegacyEclipseCreds)
if ($needCreds -and (Test-Path $Sunshine)) {
    $pass = New-UiPassword
    $p = Start-Process -FilePath $Sunshine -ArgumentList @("--creds", "eclipse", $pass) `
        -WorkingDirectory $InstallDir -Wait -PassThru -WindowStyle Hidden
    if ($p.ExitCode -eq 0) {
        Set-Content -Path $PassFile -Value $pass -Encoding utf8
        icacls $PassFile /inheritance:r /grant:r "Administrators:F" "SYSTEM:F" | Out-Null
        Write-Host "Eclipse: seeded Sunshine Web UI credentials (username eclipse). Password: $PassFile"
    }
}

# GameStream video smoke. Modes: host | client <ip> | loopback
# Pair/stream flags match src-tauri/src/session.rs. See docs/Video.md.
param(
    [Parameter(Position = 0)]
    [ValidateSet("host", "client", "loopback")]
    [string]$Mode = "host",
    [Parameter(Position = 1)]
    [string]$PeerIp = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Transcript = Join-Path $env:TEMP "eclipse-video-smoke.log"
$UiPort = 47990
$RtspPort = 48010
$StreamSecs = 10
if ($env:ECLIPSE_VIDEO_SMOKE_SECS) {
    $StreamSecs = [int]$env:ECLIPSE_VIDEO_SMOKE_SECS
}
$Resolution = "1920x1080"
if ($env:ECLIPSE_SMOKE_RESOLUTION) {
    $Resolution = $env:ECLIPSE_SMOKE_RESOLUTION
}
$PairGraceMs = 400
$PinIntervalMs = 300
$PinTries = 80
$PostPinSettleSec = 3

Set-Content -Path $Transcript -Value "" -Encoding utf8

$Triple = (rustc --print host-tuple).Trim()
$SunBin = Join-Path $Root "src-tauri\binaries\sunshine-$Triple.exe"
$MoonBin = Join-Path $Root "src-tauri\binaries\moonlight-$Triple.exe"
$Assets = Join-Path $Root "src-tauri\resources\sunshine"
$script:SunProc = $null
$script:Spawned = $false

# --- logging ---

function Write-Smoke([string]$Message) {
    Add-Content -Path $Transcript -Value $Message
    Write-Host $Message
}

function Die([string]$Message) {
    Write-Smoke "FAIL: $Message"
    Write-Smoke "Transcript: $Transcript"
    exit 1
}

function Stop-Proc($Proc) {
    if ($Proc -and -not $Proc.HasExited) {
        try { Stop-Process -Id $Proc.Id -Force -ErrorAction SilentlyContinue } catch { }
    }
}

# --- net ---

function Test-TcpOpen([string]$HostName, [int]$Port) {
    try {
        $client = New-Object System.Net.Sockets.TcpClient
        $iar = $client.BeginConnect($HostName, $Port, $null, $null)
        $ok = $iar.AsyncWaitHandle.WaitOne(1500, $false)
        if (-not $ok) { $client.Close(); return $false }
        $client.EndConnect($iar)
        $client.Close()
        return $true
    } catch {
        return $false
    }
}

function Get-HttpCode([string]$Url) {
    if (-not (Get-Command curl.exe -ErrorAction SilentlyContinue)) { return "000" }
    $code = & curl.exe -k -s -o NUL -w "%{http_code}" --max-time 3 $Url 2>$null
    if ($code -notmatch '^[0-9]{3}$') { return "000" }
    return $code
}

function Test-HttpsOk([string]$Code) {
    return [bool]($Code -match '^[1-5]\d{2}$')
}

# --- sunshine log ---

function Get-SunshineLogPath {
    $candidates = @(
        (Join-Path (Split-Path $SunBin -Parent) "config\sunshine.log"),
        (Join-Path $env:APPDATA "sunshine\sunshine.log"),
        (Join-Path $env:LOCALAPPDATA "Sunshine\config\sunshine.log")
    )
    foreach ($p in $candidates) {
        if (Test-Path $p) { return $p }
    }
    return $candidates[0]
}

function Get-LatestSlice([string]$File) {
    if (-not (Test-Path $File)) { return "" }
    $lines = Get-Content $File
    $start = 0
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "Sunshine version:") { $start = $i }
    }
    return ($lines[$start..($lines.Count - 1)] -join "`n")
}

function Get-HostVideoIssue([string]$Text) {
    if ($Text -match "config: 'capture' = kms" -and $Text -match "Failed to gain CAP_SYS_ADMIN") {
        return "KMS capture lacks CAP_SYS_ADMIN (Linux-only). See docs/Video.md."
    }
    if ($Text -match "Unable to initialize capture method|Platform failed to initialize|Unable to find display or encoder|Video failed to find working encoder") {
        return "Capture/encoder did not initialize. See docs/Video.md."
    }
    return $null
}

function Write-HostSummary([string]$Slice) {
    Write-Smoke "--- Sunshine log (latest instance) ---"
    $Slice -split "`n" | Where-Object {
        $_ -match "Found H\.264 encoder|Found HEVC|Found AV1|Desktop duplication|DXGI|CLIENT CONNECTED|Couldn't import RGB|Could not convert image|Unable to initialize capture|Creating encoder \[h264"
    } | Select-Object -Last 40 | ForEach-Object { Write-Smoke $_ }
}

# --- host ---

function Stop-SpawnedSunshine {
    if ($script:Spawned) { Stop-Proc $script:SunProc }
}

function Ensure-HostUp {
    $url = "https://127.0.0.1:$UiPort"
    if (Test-TcpOpen "127.0.0.1" $UiPort) {
        $code = Get-HttpCode $url
        if (-not (Test-HttpsOk $code)) {
            Die "Port $UiPort is open but HTTPS $url did not respond (HTTP $code)."
        }
        Write-Smoke "Attaching to Sunshine already on $url (HTTP $code)"
        return
    }
    if (-not (Test-Path $SunBin)) {
        Die "Missing $SunBin. Run .\scripts\build-sidecars.ps1"
    }
    Write-Smoke "Spawning $SunBin"
    $dir = Split-Path $SunBin -Parent
    if (Test-Path $Assets) { $env:SUNSHINE_ASSETS_DIR = $Assets }
    $script:SunProc = Start-Process -FilePath $SunBin -WorkingDirectory $dir -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput (Join-Path $env:TEMP "eclipse-sunshine-stdout.log") `
        -RedirectStandardError (Join-Path $env:TEMP "eclipse-sunshine-stderr.log")
    $script:Spawned = $true
    for ($i = 0; $i -lt 40; $i++) {
        $code = Get-HttpCode $url
        if (Test-HttpsOk $code) {
            Write-Smoke "OK Sunshine HTTPS $url -> HTTP $code"
            return
        }
        if ($script:SunProc.HasExited) {
            Die "Sunshine exited before opening $url. See $Transcript"
        }
        Start-Sleep -Milliseconds 500
    }
    Die "Sunshine did not answer $url within 20s. See $Transcript"
}

function Wait-ForEncoder {
    $slog = Get-SunshineLogPath
    for ($i = 0; $i -lt 60; $i++) {
        if (Test-Path $slog) {
            $slice = Get-LatestSlice $slog
            $issue = Get-HostVideoIssue $slice
            if ($issue) { Die $issue }
            if ($slice -match "Found H\.264 encoder") { return }
        }
        if ($script:Spawned -and $script:SunProc -and $script:SunProc.HasExited) {
            Die "Sunshine exited before finding an encoder. See $Transcript"
        }
        Start-Sleep -Milliseconds 500
    }
    Die "No H.264 encoder line in $slog after 30s. See docs/Video.md."
}

function Assert-HostPorts {
    if (-not (Test-TcpOpen "127.0.0.1" $UiPort)) { Die "TCP $UiPort not listening" }
    if (-not (Test-TcpOpen "127.0.0.1" $RtspPort)) { Die "TCP $RtspPort (RTSP) not listening" }
    try {
        $ui = Get-NetTCPConnection -LocalPort $UiPort -State Listen -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty OwningProcess
        $rtsp = Get-NetTCPConnection -LocalPort $RtspPort -State Listen -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty OwningProcess
        if ($ui -and $rtsp -and $ui -ne $rtsp) {
            Die "Web UI :$UiPort is pid $ui but RTSP :$RtspPort is pid $rtsp. Kill extra Sunshine, restart Eclipse."
        }
        if (@(Get-Process -Name sunshine -ErrorAction SilentlyContinue).Count -gt 1) {
            Die "Multiple Sunshine processes. Kill leftovers, restart Eclipse, then retry."
        }
        if ($ui) {
            $procStart = (Get-Process -Id $ui -ErrorAction SilentlyContinue).StartTime
            $logLine = (Select-String -Path (Get-SunshineLogPath) -Pattern "Sunshine version:" | Select-Object -Last 1).Line
            if ($procStart -and $logLine -match '(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})') {
                $logStart = [datetime]::ParseExact($Matches[1], "yyyy-MM-dd HH:mm:ss", $null)
                if ($procStart -lt $logStart.AddSeconds(-5)) {
                    Die "Sunshine pid $ui started before the latest log instance. Leftover host owns GameStream ports. Kill it, restart Eclipse, then retry."
                }
            }
        }
    } catch { }
}

function Invoke-HostCheck {
    Ensure-HostUp
    Wait-ForEncoder
    $slog = Get-SunshineLogPath
    if (-not (Test-Path $slog)) { Die "No Sunshine log at $slog" }
    $slice = Get-LatestSlice $slog
    Write-HostSummary $slice
    Assert-HostPorts
    Write-Smoke "OK host: encoder present, TCP $UiPort and $RtspPort open"
}

# --- moonlight ---

function Get-MoonLogText([string[]]$Paths) {
    $chunks = @()
    foreach ($p in $Paths) {
        if (Test-Path $p) { $chunks += (Get-Content $p -Raw -ErrorAction SilentlyContinue) }
    }
    return ($chunks -join "`n")
}

function Test-ListHasDesktop([string]$Ip) {
    if (-not (Test-Path $MoonBin)) {
        Die "Missing $MoonBin. Run .\scripts\build-sidecars.ps1"
    }
    $out = & $MoonBin list $Ip 2>&1 | Out-String
    Add-Content -Path $Transcript -Value $out
    return [bool]($out -match "(?m)^\s*Desktop\s*$")
}

function Submit-Pin([string]$Ip, [string]$Pin) {
    $url = "https://${Ip}:${UiPort}/api/pin"
    $pinFile = Join-Path $env:TEMP "eclipse-pin-smoke.json"
    $code = & curl.exe -k -s -o $pinFile -w "%{http_code}" --max-time 5 `
        -H "Content-Type: application/json" `
        -d "{`"pin`":`"$Pin`",`"name`":`"Eclipse`"}" $url 2>$null
    if ($code -notmatch '^[0-9]{3}$') { return "000" }
    return $code
}

function Invoke-TryPair([string]$Ip, [string]$Pin) {
    $pairLog = Join-Path $env:TEMP "eclipse-moonlight-pair.log"
    $pairErr = Join-Path $env:TEMP "eclipse-moonlight-pair.err"
    Set-Content -Path $pairLog -Value "" -Encoding utf8
    Set-Content -Path $pairErr -Value "" -Encoding utf8
    Write-Smoke "Pairing $Ip with PIN $Pin"
    $pair = Start-Process -FilePath $MoonBin -ArgumentList @("pair", $Ip, "--pin", $Pin) `
        -RedirectStandardOutput $pairLog -RedirectStandardError $pairErr -PassThru -NoNewWindow
    Start-Sleep -Milliseconds $PairGraceMs
    if ($pair.HasExited) {
        Add-Content -Path $Transcript -Value (Get-MoonLogText @($pairLog, $pairErr))
        Write-Smoke "Moonlight pair exited before PIN POST"
        return $false
    }
    for ($i = 0; $i -lt $PinTries; $i++) {
        $code = Submit-Pin $Ip $Pin
        if ($code -eq "401") {
            Stop-Proc $pair
            Die "PIN POST 401: host sidecar is missing Eclipse's CGNAT pairing patch. Rebuild sidecars on the host."
        }
        if ($pair.HasExited) { break }
        Start-Sleep -Milliseconds $PinIntervalMs
    }
    Start-Sleep -Seconds $PostPinSettleSec
    Stop-Proc $pair
    Add-Content -Path $Transcript -Value (Get-MoonLogText @($pairLog, $pairErr))
    return (Test-ListHasDesktop $Ip)
}

function Pair-IfNeeded([string]$Ip) {
    if (Test-ListHasDesktop $Ip) {
        Write-Smoke "Already paired with $Ip (Desktop listed)"
        return
    }
    for ($attempt = 1; $attempt -le 2; $attempt++) {
        $pin = "{0:D4}" -f (Get-Random -Maximum 10000)
        if (Invoke-TryPair $Ip $pin) {
            Write-Smoke "Paired: Desktop listed"
            return
        }
        Write-Smoke "Pair attempt $attempt did not list Desktop"
    }
    Die "Moonlight never listed Desktop after pairing. See $Transcript"
}

function Assert-MoonLog([string]$Text, [string]$Ip) {
    if ($Text -match "has not been paired|Incorrect PIN") {
        Die "Moonlight is not paired with $Ip. Pairing must list Desktop before streaming."
    }
    if ($Text -match "No video received|Failed to initialise video capture|error 503") {
        Die "Moonlight reported no video. See $Transcript"
    }
}

function Invoke-Client([string]$Ip) {
    if (-not $Ip) { Die "client mode needs an IP" }
    if (-not (Test-Path $MoonBin)) {
        Die "Missing $MoonBin. Run .\scripts\build-sidecars.ps1"
    }
    Pair-IfNeeded $Ip
    $slog = Get-SunshineLogPath
    $mark = 0
    if (Test-Path $slog) {
        $mark = (Get-Content $slog | Measure-Object -Line).Lines
    }
    $moonLog = Join-Path $env:TEMP "eclipse-moonlight-smoke.log"
    $moonErr = Join-Path $env:TEMP "eclipse-moonlight-smoke.err"
    Set-Content -Path $moonLog -Value "" -Encoding utf8
    Set-Content -Path $moonErr -Value "" -Encoding utf8
    Write-Smoke "Streaming to $Ip for ${StreamSecs}s (windowed $Resolution H.264)"
    $moon = Start-Process -FilePath $MoonBin -ArgumentList @(
        "stream", "--resolution", $Resolution, "--video-codec", "H.264", "--no-hdr", "--packet-size", "1024",
        "--display-mode", "windowed", $Ip, "Desktop"
    ) -RedirectStandardOutput $moonLog -RedirectStandardError $moonErr -PassThru -NoNewWindow
    for ($t = 0; $t -lt $StreamSecs; $t++) {
        Start-Sleep -Seconds 1
        $moonText = Get-MoonLogText @($moonLog, $moonErr)
        if ($moonText -match "has not been paired|Incorrect PIN|No video received|Failed to initialise video capture|error 503") {
            Stop-Proc $moon
            Add-Content -Path $Transcript -Value $moonText
            Assert-MoonLog $moonText $Ip
        }
    }
    Stop-Proc $moon
    $moonText = Get-MoonLogText @($moonLog, $moonErr)
    Add-Content -Path $Transcript -Value $moonText
    Assert-MoonLog $moonText $Ip
    $new = ""
    if (Test-Path $slog) {
        $all = Get-Content $slog
        if ($all.Count -gt $mark) {
            $new = $all[$mark..($all.Count - 1)] -join "`n"
            if ($new -match "Couldn't import RGB Image") {
                Die "Sunshine could not import captured frames (EGL 00003009). See docs/Video.md."
            }
            if ($new -match "Could not convert image") {
                Die "Sunshine could not convert captured frames. See docs/Video.md."
            }
            if ($new -match "CLIENT CONNECTED") {
                Write-Smoke "Sunshine: CLIENT CONNECTED"
            }
        }
    }
    if ($Ip -in @("127.0.0.1", "localhost", "::1") -and $new -notmatch "CLIENT CONNECTED") {
        Die "Loopback stream ran ${StreamSecs}s but Sunshine never logged CLIENT CONNECTED."
    }
    Write-Smoke "OK client: Moonlight ran ${StreamSecs}s without a no-video error"
}

# --- main ---

try {
    switch ($Mode) {
        "host" { Invoke-HostCheck }
        "client" { Invoke-Client $PeerIp }
        "loopback" {
            Invoke-HostCheck
            Invoke-Client "127.0.0.1"
        }
    }
    Write-Smoke "Transcript: $Transcript"
} finally {
    Stop-SpawnedSunshine
}

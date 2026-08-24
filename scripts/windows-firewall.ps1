# Optional GameStream Windows Firewall helper. Video failures with working audio
# are usually encoder/resolution, not these ports. Use -Remove to delete rules.
param(
    [switch]$SkipElevate,
    [switch]$Remove
)

$ErrorActionPreference = "Stop"

$TcpName = "Eclipse GameStream TCP (in)"
$UdpName = "Eclipse GameStream UDP (in)"

function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $p = New-Object Security.Principal.WindowsPrincipal($id)
    return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Remove-EclipseFirewallRule([string]$Name) {
    Get-NetFirewallRule -DisplayName $Name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
}

function Install-EclipseFirewallRules {
    Remove-EclipseFirewallRule $TcpName
    Remove-EclipseFirewallRule $UdpName

    New-NetFirewallRule -DisplayName $TcpName -Direction Inbound -Action Allow -Protocol TCP `
        -LocalPort 47984-47990,48010 -Profile Any -Enabled True | Out-Null
    New-NetFirewallRule -DisplayName $UdpName -Direction Inbound -Action Allow -Protocol UDP `
        -LocalPort 47998-48010 -Profile Any -Enabled True | Out-Null

    Write-Host "Windows Firewall: allowed inbound TCP 47984-47990,48010 and UDP 47998-48010 (all profiles)."
}

function Uninstall-EclipseFirewallRules {
    Remove-EclipseFirewallRule $TcpName
    Remove-EclipseFirewallRule $UdpName
    Write-Host "Removed Eclipse GameStream firewall rules."
}

if (-not (Test-Admin)) {
    if ($SkipElevate) {
        Write-Host "Skipping GameStream firewall rules (not Administrator)."
        exit 0
    }
    Write-Host "Eclipse needs Administrator once to change GameStream firewall rules."
    $childArgs = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File",
        $PSCommandPath,
        "-SkipElevate"
    )
    if ($Remove) {
        $childArgs += "-Remove"
    }
    $p = Start-Process -FilePath "powershell.exe" -Verb RunAs -Wait -PassThru -ArgumentList $childArgs
    if ($p.ExitCode -ne 0) {
        throw "GameStream firewall rules failed (exit $($p.ExitCode))."
    }
    exit 0
}

if ($Remove) {
    Uninstall-EclipseFirewallRules
} else {
    Install-EclipseFirewallRules
}

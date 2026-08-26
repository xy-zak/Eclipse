# Emit Eclipse sidecar patches from a dirty vendor tree (Git Bash).
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$bash = Get-Command bash -ErrorAction SilentlyContinue
if (-not $bash) {
    throw "bash not found. Install Git for Windows and re-run, or run scripts/refresh-sidecar-patches.sh"
}
& $bash.Source (Join-Path $here "refresh-sidecar-patches.sh")
if ($LASTEXITCODE -and $LASTEXITCODE -ne 0) {
    throw "refresh-sidecar-patches.sh failed (exit $LASTEXITCODE)"
}

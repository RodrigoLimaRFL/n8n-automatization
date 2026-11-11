# Starts n8n in the background from this project, logging to logs\n8n.log.
# Used by the "n8n autostart" Windows task (see scripts\install-autostart.ps1).
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

New-Item -ItemType Directory -Force logs | Out-Null
$log = Join-Path $root 'logs\n8n.log'
$bridgeLog = Join-Path $root 'logs\claude-bridge.log'
# Simple rotation: keep each log under 5 MB, plus one older copy.
foreach ($f in $log, $bridgeLog) {
  if ((Test-Path $f) -and (Get-Item $f).Length -gt 5MB) { Move-Item $f "$f.old" -Force }
}

$node = (Get-Command node -ErrorAction SilentlyContinue).Source
if (-not $node) { Add-Content $log "$(Get-Date -Format s) node.exe not found on PATH"; exit 1 }

# Claude bridge (port 5681): lets workflows use Claude through your Claude plan.
if (-not (Get-NetTCPConnection -LocalPort 5681 -State Listen -ErrorAction SilentlyContinue)) {
  Start-Process -FilePath 'cmd.exe' -WindowStyle Hidden -WorkingDirectory $root -ArgumentList @(
    '/d', '/c', "`"`"$node`" --env-file=.env scripts\claude-bridge.mjs >> `"$bridgeLog`" 2>&1`""
  )
}

# n8n already running? Port 5678 in use means yes: nothing more to do.
if (Get-NetTCPConnection -LocalPort 5678 -State Listen -ErrorAction SilentlyContinue) { exit 0 }

Add-Content $log "`r`n===== $(Get-Date -Format s) starting n8n ====="
# cmd handles the redirection (Windows PowerShell 5.1 mangles native stderr).
cmd.exe /d /c "`"$node`" --env-file=.env node_modules\n8n\bin\n8n start >> `"$log`" 2>&1"
exit $LASTEXITCODE

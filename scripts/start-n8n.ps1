# Starts n8n in the background from this project, logging to logs\n8n.log.
# Used by the "n8n autostart" Windows task (see scripts\install-autostart.ps1).
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# Already running? Port 5678 in use means yes: do nothing.
if (Get-NetTCPConnection -LocalPort 5678 -State Listen -ErrorAction SilentlyContinue) { exit 0 }

New-Item -ItemType Directory -Force logs | Out-Null
$log = Join-Path $root 'logs\n8n.log'
# Simple rotation: keep the current log under 5 MB, plus one older copy.
if ((Test-Path $log) -and (Get-Item $log).Length -gt 5MB) { Move-Item $log "$log.old" -Force }

$node = (Get-Command node -ErrorAction SilentlyContinue).Source
if (-not $node) { Add-Content $log "$(Get-Date -Format s) node.exe not found on PATH"; exit 1 }

Add-Content $log "`r`n===== $(Get-Date -Format s) starting n8n ====="
# cmd handles the redirection (Windows PowerShell 5.1 mangles native stderr).
cmd.exe /d /c "`"$node`" --env-file=.env node_modules\n8n\bin\n8n start >> `"$log`" 2>&1"
exit $LASTEXITCODE

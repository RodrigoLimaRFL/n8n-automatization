# Restarts n8n and the Claude bridge in the background (hidden), e.g. after editing .env.
# Uses the "n8n autostart" task when it exists, so the process isn't tied to this terminal.
& (Join-Path $PSScriptRoot 'stop-n8n.ps1')
Start-Sleep -Seconds 2

if (Get-ScheduledTask -TaskName 'n8n autostart' -ErrorAction SilentlyContinue) {
  Start-ScheduledTask -TaskName 'n8n autostart'
} else {
  Start-Process powershell.exe -WindowStyle Hidden -ArgumentList @(
    '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$(Join-Path $PSScriptRoot 'start-n8n.ps1')`""
  )
}

# Wait until the editor answers (up to ~2 min).
for ($i = 0; $i -lt 60; $i++) {
  Start-Sleep -Seconds 2
  try {
    if ((Invoke-WebRequest -UseBasicParsing -TimeoutSec 2 http://localhost:5678/healthz).StatusCode -eq 200) {
      Write-Output 'n8n is running in the background: http://localhost:5678'
      exit 0
    }
  } catch { }
}
Write-Output 'n8n did not answer within 2 minutes. Check logs\n8n.log.'
exit 1

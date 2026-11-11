# Stops whatever n8n is listening on port 5678 (e.g. the hidden autostart instance).
$conns = Get-NetTCPConnection -LocalPort 5678 -State Listen -ErrorAction SilentlyContinue
if (-not $conns) { Write-Output 'n8n is not running.'; exit 0 }
foreach ($id in ($conns.OwningProcess | Sort-Object -Unique)) {
  $p = Get-CimInstance Win32_Process -Filter "ProcessId=$id"
  if ($p.Name -ne 'node.exe') { Write-Output "Port 5678 is used by $($p.Name) (pid $id), not n8n. Leaving it alone."; continue }
  Stop-Process -Id $id -Force
  Write-Output "Stopped n8n (pid $id)."
}

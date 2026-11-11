# Stops n8n (port 5678) and the Claude bridge (port 5681), e.g. the hidden autostart instances.
foreach ($port in 5678, 5681) {
  $label = if ($port -eq 5678) { 'n8n' } else { 'Claude bridge' }
  $conns = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
  if (-not $conns) { Write-Output "$label is not running."; continue }
  foreach ($id in ($conns.OwningProcess | Sort-Object -Unique)) {
    $p = Get-CimInstance Win32_Process -Filter "ProcessId=$id"
    if ($p.Name -ne 'node.exe') { Write-Output "Port $port is used by $($p.Name) (pid $id), not $label. Leaving it alone."; continue }
    Stop-Process -Id $id -Force
    Write-Output "Stopped $label (pid $id)."
  }
}

# Removes the "n8n autostart" task. Does not stop a running n8n (use scripts\stop-n8n.ps1).
$taskName = 'n8n autostart'
if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
  Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
  Write-Output "Removed task '$taskName'."
} else {
  Write-Output "Task '$taskName' does not exist."
}

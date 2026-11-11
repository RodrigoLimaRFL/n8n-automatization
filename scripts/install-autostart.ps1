# Registers a Windows Task Scheduler task that starts n8n (hidden) when you log in.
# Runs as your user, no admin needed. Safe to run again: it replaces the existing task.
$taskName = 'n8n autostart'
$script = Join-Path $PSScriptRoot 'start-n8n.ps1'

$action = New-ScheduledTaskAction -Execute 'powershell.exe' `
  -Argument "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$script`"" `
  -WorkingDirectory (Split-Path -Parent $PSScriptRoot)

# At logon, 30 s later, so Wi-Fi has a moment to connect.
$trigger = New-ScheduledTaskTrigger -AtLogOn -User "$env:USERDOMAIN\$env:USERNAME"
$trigger.Delay = 'PT30S'

$settings = New-ScheduledTaskSettingsSet `
  -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
  -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) `
  -MultipleInstances IgnoreNew

$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -LogonType Interactive -RunLevel Limited

Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Settings $settings `
  -Principal $principal -Description 'Starts n8n from the n8n-automatization project at logon.' -Force | Out-Null

Write-Output "Registered task '$taskName'. n8n will start ~30 s after you log in."
Write-Output "Start it now with:  Start-ScheduledTask -TaskName '$taskName'"

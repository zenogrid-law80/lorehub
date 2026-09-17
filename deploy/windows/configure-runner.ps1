param(
    [string]$InstallDirectory = "$env:ProgramFiles\LoreHub",
    [string]$DataDirectory = "$env:ProgramData\LoreHub",
    [string]$TaskName = "LoreHub Runner"
)

$ErrorActionPreference = "Stop"

$runner = Join-Path $InstallDirectory "lorehub.exe"
$lore = Join-Path $InstallDirectory "lore.exe"
$launcher = Join-Path $InstallDirectory "run-runner.ps1"
$environmentExample = Join-Path $InstallDirectory "runner.env.example"
$environmentFile = Join-Path $DataDirectory "runner.env"

foreach ($path in @($runner, $lore, $launcher, $environmentExample)) {
    if (-not (Test-Path $path)) { throw "Missing installed Runner file: $path" }
}

New-Item -ItemType Directory -Force -Path $DataDirectory, (Join-Path $DataDirectory "work") | Out-Null
if (-not (Test-Path $environmentFile)) {
    Copy-Item $environmentExample $environmentFile
}

# Runner configuration can contain signing-key paths.
& icacls.exe $DataDirectory /inheritance:r /grant:r "SYSTEM:(OI)(CI)F" "BUILTIN\Administrators:(OI)(CI)F" | Out-Null

$existing = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if ($existing) {
    Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
    Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
}
$arguments = "-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$launcher`" -DataDirectory `"$DataDirectory`""
$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument $arguments
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit ([TimeSpan]::Zero)
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null

Write-Host "LoreHub Runner installed. Edit $environmentFile, then start the '$TaskName' scheduled task."

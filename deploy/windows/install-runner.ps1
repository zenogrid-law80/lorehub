param(
    [string]$Binary = ".\lorehub.exe",
    [string]$LoreBinary = ".\lore.exe",
    [string]$LoreInstallDirectory = "$env:ProgramFiles\Lore",
    [string]$InstallDirectory = "$env:ProgramFiles\LoreHub",
    [string]$DataDirectory = "$env:ProgramData\LoreHub",
    [string]$TaskName = "LoreHub Runner"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $Binary)) { throw "LoreHub binary not found: $Binary" }
if (-not (Test-Path $LoreBinary)) { throw "Lore binary not found: $LoreBinary" }
New-Item -ItemType Directory -Force -Path $InstallDirectory, $LoreInstallDirectory, $DataDirectory | Out-Null
Copy-Item -Force $Binary (Join-Path $InstallDirectory "lorehub.exe")
Copy-Item -Force $LoreBinary (Join-Path $LoreInstallDirectory "lore.exe")
Copy-Item -Force (Join-Path $PSScriptRoot "run-runner.ps1") $InstallDirectory

$environmentFile = Join-Path $DataDirectory "runner.env"
if (-not (Test-Path $environmentFile)) {
    Copy-Item (Join-Path $PSScriptRoot "runner.env.example") $environmentFile
}

$acl = Get-Acl $DataDirectory
$acl.SetAccessRuleProtection($true, $false)
$acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule("SYSTEM", "FullControl", "ContainerInherit,ObjectInherit", "None", "Allow")))
$acl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule("Administrators", "FullControl", "ContainerInherit,ObjectInherit", "None", "Allow")))
Set-Acl $DataDirectory $acl

$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$InstallDirectory\run-runner.ps1`" -DataDirectory `"$DataDirectory`""
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1) -ExecutionTimeLimit ([TimeSpan]::Zero)
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null

Write-Host "Edit $environmentFile, then run: Start-ScheduledTask -TaskName '$TaskName'"

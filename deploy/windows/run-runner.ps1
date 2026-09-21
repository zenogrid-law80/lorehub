param([string]$DataDirectory = "$env:ProgramData\LoreHub")

$ErrorActionPreference = "Stop"

$environmentFile = Join-Path $dataDirectory "runner.env"
$binary = Join-Path $PSScriptRoot "lorehub.exe"
$previousBinary = "$binary.previous"

if (-not (Test-Path $environmentFile)) {
    throw "Missing runner environment file: $environmentFile"
}
if (-not (Test-Path $binary) -and (Test-Path $previousBinary)) {
    Move-Item $previousBinary $binary
}
if (-not (Test-Path $binary)) {
    throw "Missing LoreHub runner binary: $binary"
}

Get-Content $environmentFile | ForEach-Object {
    $line = $_.Trim()
    if ($line -and -not $line.StartsWith("#")) {
        $parts = $line.Split("=", 2)
        if ($parts.Count -ne 2) { throw "Invalid environment entry: $line" }
        $value = [Environment]::ExpandEnvironmentVariables($parts[1])
        [Environment]::SetEnvironmentVariable($parts[0].Trim(), $value, "Process")
    }
}

$retryIntervalSeconds = 60
if ($env:LOREHUB_RETRY_INTERVAL_SECONDS) {
    $parsedRetryInterval = 0
    if (-not [int]::TryParse($env:LOREHUB_RETRY_INTERVAL_SECONDS, [ref]$parsedRetryInterval) -or $parsedRetryInterval -lt 5) {
        throw "LOREHUB_RETRY_INTERVAL_SECONDS must be an integer of at least 5 seconds"
    }
    $retryIntervalSeconds = $parsedRetryInterval
}

while ($true) {
    & $binary worker --work-dir (Join-Path $dataDirectory "work")
    $runnerExitCode = $LASTEXITCODE
    if ($runnerExitCode -eq 75) {
        $stagedBinary = "$binary.update"
        if (-not (Test-Path $stagedBinary)) {
            throw "Runner requested an update restart, but the staged binary is missing: $stagedBinary"
        }
        if (Test-Path $previousBinary) {
            Remove-Item -Force $previousBinary
        }
        Move-Item $binary $previousBinary
        try {
            Move-Item $stagedBinary $binary
        }
        catch {
            Move-Item $previousBinary $binary
            throw
        }
        Remove-Item -Force $previousBinary
        continue
    }

    Write-Warning "LoreHub Runner exited with code $runnerExitCode. Retrying in $retryIntervalSeconds seconds."
    Start-Sleep -Seconds $retryIntervalSeconds
}

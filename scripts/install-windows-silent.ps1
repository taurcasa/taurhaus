param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,

    [Parameter(Mandatory = $true)]
    [string]$BuiltExePath,

    [string]$InstalledExePath = "$env:LOCALAPPDATA\taurhaus\taurhaus.exe"
)

$ErrorActionPreference = "Stop"

function Stop-TaurhausProcesses {
    $processes = Get-Process -Name "taurhaus" -ErrorAction SilentlyContinue
    if (-not $processes) {
        return
    }

    Write-Host "Stopping running taurhaus.exe instances before silent install..."
    & taskkill /IM taurhaus.exe /F /T | Out-Null
    $processes | Stop-Process -Force -ErrorAction SilentlyContinue

    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        $remaining = @(Get-Process -Name "taurhaus" -ErrorAction SilentlyContinue | Where-Object { -not $_.HasExited })
        if (-not $remaining) {
            return
        }
        Start-Sleep -Milliseconds 100
    }

    $remainingIds = (@(Get-Process -Name "taurhaus" -ErrorAction SilentlyContinue | Where-Object { -not $_.HasExited } | Select-Object -ExpandProperty Id)) -join ", "
    throw "Failed to stop running taurhaus.exe instances before silent install. remaining_pids=$remainingIds"
}

function Get-BunPath {
    $bunBinDir = Join-Path $env:USERPROFILE ".bun\bin"
    $bunFallback = Join-Path $bunBinDir "bun.exe"
    $bunCommand = Get-Command bun -ErrorAction SilentlyContinue
    if ($bunCommand) {
        return $bunCommand.Source
    }
    if (Test-Path -LiteralPath $bunFallback) {
        return $bunFallback
    }
    throw "bun not found on PATH and %USERPROFILE%\.bun\bin\bun.exe is missing"
}

$InstallAttemptId = [guid]::NewGuid().ToString("N")
$InstalledExeBackupPath = "$InstalledExePath.preinstall-backup.$InstallAttemptId"

function Move-InstalledExeAside {
    if (-not (Test-Path -LiteralPath $InstalledExePath)) {
        return
    }

    Move-Item -LiteralPath $InstalledExePath -Destination $InstalledExeBackupPath -Force
}

function Restore-InstalledExeBackup {
    if ((-not (Test-Path -LiteralPath $InstalledExePath)) -and (Test-Path -LiteralPath $InstalledExeBackupPath)) {
        Move-Item -LiteralPath $InstalledExeBackupPath -Destination $InstalledExePath -Force
    }
}

function Remove-InstalledExeBackup {
    if (Test-Path -LiteralPath $InstalledExeBackupPath) {
        Remove-Item -LiteralPath $InstalledExeBackupPath -Force -ErrorAction SilentlyContinue
    }
}

if (-not (Test-Path -LiteralPath $InstallerPath)) {
    throw "Installer not found: $InstallerPath"
}

if (-not (Test-Path -LiteralPath $BuiltExePath)) {
    throw "Built exe not found: $BuiltExePath"
}

Stop-TaurhausProcesses
Move-InstalledExeAside

try {
    Start-Process -FilePath $InstallerPath -ArgumentList "/S" -Wait

    if (-not (Test-Path -LiteralPath $InstalledExePath)) {
        throw "Installed exe not found after silent install: $InstalledExePath"
    }

    $bundledHashScript = Join-Path $PSScriptRoot "windows-nsis-payload-hash.mjs"
    if (-not (Test-Path -LiteralPath $bundledHashScript)) {
        throw "NSIS payload hash helper not found: $bundledHashScript"
    }

    $bunPath = Get-BunPath
    $expectedInstalledHash = (& $bunPath $bundledHashScript $BuiltExePath).Trim()
    if (-not $expectedInstalledHash) {
        throw "Failed to derive expected NSIS payload hash from $BuiltExePath"
    }

    $builtHash = (Get-FileHash -LiteralPath $BuiltExePath -Algorithm SHA256).Hash
    $installedHash = (Get-FileHash -LiteralPath $InstalledExePath -Algorithm SHA256).Hash

    if ($expectedInstalledHash -ne $installedHash) {
        throw "Installed exe hash mismatch after silent install. expected_installed=$expectedInstalledHash raw_built=$builtHash installed=$installedHash"
    }

    Remove-InstalledExeBackup

    Get-Item -LiteralPath $InstalledExePath |
        Select-Object FullName, LastWriteTime, Length
}
catch {
    Restore-InstalledExeBackup
    throw
}

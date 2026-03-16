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
    $processes | Stop-Process -Force -ErrorAction SilentlyContinue

    for ($attempt = 0; $attempt -lt 50; $attempt++) {
        $remaining = Get-Process -Name "taurhaus" -ErrorAction SilentlyContinue
        if (-not $remaining) {
            return
        }
        Start-Sleep -Milliseconds 100
    }

    $remainingIds = (Get-Process -Name "taurhaus" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id) -join ", "
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

if (-not (Test-Path -LiteralPath $InstallerPath)) {
    throw "Installer not found: $InstallerPath"
}

if (-not (Test-Path -LiteralPath $BuiltExePath)) {
    throw "Built exe not found: $BuiltExePath"
}

Stop-TaurhausProcesses

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

Get-Item -LiteralPath $InstalledExePath |
    Select-Object FullName, LastWriteTime, Length

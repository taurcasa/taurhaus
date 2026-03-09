param(
    [Parameter(Mandatory = $true)]
    [string]$InstallerPath,

    [Parameter(Mandatory = $true)]
    [string]$BuiltExePath,

    [string]$InstalledExePath = "$env:LOCALAPPDATA\taurhaus\taurhaus.exe"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $InstallerPath)) {
    throw "Installer not found: $InstallerPath"
}

if (-not (Test-Path -LiteralPath $BuiltExePath)) {
    throw "Built exe not found: $BuiltExePath"
}

Start-Process -FilePath $InstallerPath -ArgumentList "/S" -Wait

if (-not (Test-Path -LiteralPath $InstalledExePath)) {
    throw "Installed exe not found after silent install: $InstalledExePath"
}

$builtHash = (Get-FileHash -LiteralPath $BuiltExePath -Algorithm SHA256).Hash
$installedHash = (Get-FileHash -LiteralPath $InstalledExePath -Algorithm SHA256).Hash

if ($builtHash -ne $installedHash) {
    throw "Installed exe hash mismatch after silent install. built=$builtHash installed=$installedHash"
}

Get-Item -LiteralPath $InstalledExePath |
    Select-Object FullName, LastWriteTime, Length

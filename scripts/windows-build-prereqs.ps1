param(
    [switch]$CheckOnly,
    [switch]$Install,
    [string]$BunVersion = "",
    [string]$RustToolchain = "stable-x86_64-pc-windows-msvc",
    [string]$LogPath = "",
    [switch]$PauseOnError
)

$ErrorActionPreference = "Stop"

if (-not $LogPath) {
    $logDir = Join-Path $env:LOCALAPPDATA "taurhaus\logs"
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    $timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $LogPath = Join-Path $logDir ("windows-build-prereqs-" + $timestamp + ".log")
}

$transcriptStarted = $false
try {
    Start-Transcript -Path $LogPath -Force | Out-Null
    $transcriptStarted = $true
}
catch {
    Write-Warning ("Could not start transcript logging at " + $LogPath + ": " + $_.Exception.Message)
}

if (-not $CheckOnly -and -not $Install) {
    $CheckOnly = $true
}

$bunPackageId = "Oven-sh.Bun"
$rustupPackageId = "Rustlang.Rustup"
$vsBuildToolsPackageId = "Microsoft.VisualStudio.2022.BuildTools"
$nsisPackageId = "NSIS.NSIS"
$vcToolsComponentId = "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
$windowsSdkComponentId = "Microsoft.VisualStudio.Component.Windows11SDK.22621"
$cargoBinDir = Join-Path $env:USERPROFILE ".cargo\bin"
$bunBinDir = Join-Path $env:USERPROFILE ".bun\bin"

function Test-IsAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Add-PathEntryIfPresent {
    param([string]$PathEntry)

    if (-not $PathEntry) {
        return
    }
    if (-not (Test-Path -LiteralPath $PathEntry)) {
        return
    }

    $currentEntries = @($env:PATH -split ";")
    if ($currentEntries -contains $PathEntry) {
        return
    }

    $env:PATH = $PathEntry + ";" + $env:PATH
}

function Get-ExecutablePath {
    param(
        [string[]]$Names,
        [string[]]$Fallbacks = @()
    )

    foreach ($name in $Names) {
        $command = Get-Command $name -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($command) {
            return $command.Source
        }
    }

    foreach ($candidate in $Fallbacks) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }

    return $null
}

function Get-VsWherePath {
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"),
        (Join-Path $env:ProgramFiles "Microsoft Visual Studio\Installer\vswhere.exe")
    ) | Where-Object { $_ }

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate) {
            return $candidate
        }
    }

    return $null
}

function Get-VsBuildToolsInstall {
    $vswherePath = Get-VsWherePath
    if (-not $vswherePath) {
        return $null
    }

    $json = & $vswherePath -latest -products * -requires $vcToolsComponentId -format json
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($json)) {
        return $null
    }

    $installations = @($json | ConvertFrom-Json)
    if ($installations.Count -eq 0) {
        return $null
    }

    return $installations[0]
}

function Get-PrereqStatus {
    Add-PathEntryIfPresent $cargoBinDir
    Add-PathEntryIfPresent $bunBinDir

    $bunPath = Get-ExecutablePath -Names @("bun") -Fallbacks @(
        (Join-Path $bunBinDir "bun.exe")
    )
    $cargoPath = Get-ExecutablePath -Names @("cargo") -Fallbacks @(
        (Join-Path $cargoBinDir "cargo.exe")
    )
    $rustupPath = Get-ExecutablePath -Names @("rustup") -Fallbacks @(
        (Join-Path $cargoBinDir "rustup.exe")
    )
    $rustcPath = Get-ExecutablePath -Names @("rustc") -Fallbacks @(
        (Join-Path $cargoBinDir "rustc.exe")
    )
    $nsisPath = Get-ExecutablePath -Names @("makensis", "makensis.exe") -Fallbacks @(
        (Join-Path ${env:ProgramFiles(x86)} "NSIS\makensis.exe"),
        (Join-Path $env:ProgramFiles "NSIS\makensis.exe")
    )
    $wingetPath = Get-ExecutablePath -Names @("winget", "winget.exe")
    $vsBuildTools = Get-VsBuildToolsInstall

    $missing = New-Object System.Collections.Generic.List[string]
    if (-not $bunPath) {
        $missing.Add("bun")
    }
    if (-not $cargoPath -or -not $rustcPath -or -not $rustupPath) {
        $missing.Add("Rust MSVC toolchain (cargo, rustc, rustup)")
    }
    if (-not $vsBuildTools) {
        $missing.Add("Visual Studio C++ Build Tools")
    }
    if (-not $nsisPath) {
        $missing.Add("NSIS")
    }

    return [pscustomobject]@{
        WingetPath   = $wingetPath
        BunPath      = $bunPath
        CargoPath    = $cargoPath
        RustcPath    = $rustcPath
        RustupPath   = $rustupPath
        NsisPath     = $nsisPath
        VsBuildTools = $vsBuildTools
        Missing      = @($missing)
    }
}

function Write-PrereqStatus {
    param([pscustomobject]$Status)

    Write-Host "Windows build prerequisites:"
    Write-Host ("  bun: " + $(if ($Status.BunPath) { $Status.BunPath } else { "missing" }))
    Write-Host ("  cargo: " + $(if ($Status.CargoPath) { $Status.CargoPath } else { "missing" }))
    Write-Host ("  rustc: " + $(if ($Status.RustcPath) { $Status.RustcPath } else { "missing" }))
    Write-Host ("  rustup: " + $(if ($Status.RustupPath) { $Status.RustupPath } else { "missing" }))
    Write-Host ("  NSIS: " + $(if ($Status.NsisPath) { $Status.NsisPath } else { "missing" }))
    Write-Host ("  VS build tools: " + $(if ($Status.VsBuildTools) { $Status.VsBuildTools.installationPath } else { "missing" }))
}

function Install-WingetPackage {
    param(
        [string]$WingetPath,
        [string]$Id,
        [string]$Version = "",
        [string]$Override = "",
        [switch]$AllowVersionFallback
    )

    $baseArguments = @(
        "install",
        "--id", $Id,
        "--exact",
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--disable-interactivity"
    )

    $arguments = @($baseArguments)
    if ($Version) {
        $arguments += @("--version", $Version)
    }

    if ($Override) {
        $arguments += @("--override", $Override)
    }

    Write-Host ("Installing " + $Id + $(if ($Version) { " " + $Version } else { "" }) + "...")
    $output = & $WingetPath @arguments 2>&1
    if ($output) {
        $output | ForEach-Object { Write-Host $_ }
    }

    if ($LASTEXITCODE -eq 0) {
        return
    }

    $outputText = ($output | Out-String)
    if ($AllowVersionFallback -and $Version -and $outputText -match "No version found matching") {
        Write-Warning ("winget does not offer " + $Id + " version " + $Version + "; retrying with the latest available version")
        $retryArguments = @($baseArguments)
        if ($Override) {
            $retryArguments += @("--override", $Override)
        }
        $retryOutput = & $WingetPath @retryArguments 2>&1
        if ($retryOutput) {
            $retryOutput | ForEach-Object { Write-Host $_ }
        }
        if ($LASTEXITCODE -eq 0) {
            return
        }
        throw "winget install failed for $Id after version fallback"
    }

    throw "winget install failed for $Id"
}

function Write-ToolVersions {
    param([pscustomobject]$Status)

    Write-Host ""
    Write-Host "Verified Windows build tools:"
    & $Status.BunPath --version
    & $Status.CargoPath --version
    & $Status.RustcPath --version
    & $Status.RustupPath --version
    & $Status.NsisPath /VERSION
    if ($Status.VsBuildTools) {
        $version = $Status.VsBuildTools.catalog.productDisplayVersion
        if ($version) {
            Write-Host ("Visual Studio Build Tools " + $version)
        }
    }
}

if ($Install -and -not (Test-IsAdministrator)) {
    Write-Host "Requesting elevation to install Windows build prerequisites..."
    Write-Host ("Logging to " + $LogPath)

    $elevatedScriptPath = Join-Path $env:TEMP "taurhaus-windows-build-prereqs-elevated.ps1"
    Copy-Item -LiteralPath $PSCommandPath -Destination $elevatedScriptPath -Force

    $argumentParts = @(
        "-NoLogo",
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", ('"{0}"' -f $elevatedScriptPath),
        "-Install",
        "-RustToolchain", ('"{0}"' -f $RustToolchain),
        "-LogPath", ('"{0}"' -f $LogPath)
    )
    if ($BunVersion) {
        $argumentParts += @("-BunVersion", ('"{0}"' -f $BunVersion))
    }

    $process = Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList ($argumentParts -join " ") -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        Write-Host ("Elevated install failed. See log: " + $LogPath)
    }
    if ($transcriptStarted) {
        Stop-Transcript | Out-Null
    }
    exit $process.ExitCode
}

try {
    Write-Host ("Logging to " + $LogPath)
    $status = Get-PrereqStatus

    if ($Install) {
        Write-PrereqStatus $status
        Write-Host ""

        if (-not $status.WingetPath) {
            throw "winget is required to install Windows build prerequisites automatically"
        }

        if (-not $status.BunPath) {
            Install-WingetPackage -WingetPath $status.WingetPath -Id $bunPackageId -Version $BunVersion -AllowVersionFallback
        }

        if (-not $status.RustupPath) {
            Install-WingetPackage -WingetPath $status.WingetPath -Id $rustupPackageId
        }

        Add-PathEntryIfPresent $cargoBinDir
        Add-PathEntryIfPresent $bunBinDir

        $rustupPath = Get-ExecutablePath -Names @("rustup") -Fallbacks @(
            (Join-Path $cargoBinDir "rustup.exe")
        )
        if (-not $rustupPath) {
            throw "rustup is still unavailable after installation"
        }

        & $rustupPath default $RustToolchain
        if ($LASTEXITCODE -ne 0) {
            throw "rustup default failed for $RustToolchain"
        }

        & $rustupPath target add x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) {
            throw "rustup target add x86_64-pc-windows-msvc failed"
        }

        if (-not $status.VsBuildTools) {
            Install-WingetPackage `
                -WingetPath $status.WingetPath `
                -Id $vsBuildToolsPackageId `
                -Override ("--quiet --wait --norestart --add " + $vcToolsComponentId + " --add " + $windowsSdkComponentId + " --includeRecommended")
        }

        if (-not $status.NsisPath) {
            Install-WingetPackage -WingetPath $status.WingetPath -Id $nsisPackageId
        }

        $status = Get-PrereqStatus
        Write-Host ""
        Write-PrereqStatus $status

        if ($status.Missing.Count -gt 0) {
            throw ("Windows build prerequisites are still missing: " + ($status.Missing -join ", "))
        }

        Write-ToolVersions $status
        if ($transcriptStarted) {
            Stop-Transcript | Out-Null
        }
        exit 0
    }

    Write-PrereqStatus $status
    if ($status.Missing.Count -gt 0) {
        Write-Host ""
        Write-Host ("Missing: " + ($status.Missing -join ", "))
        Write-Host "Run `just install-windows-build-prereqs` to bootstrap the native Windows toolchain."
        if ($transcriptStarted) {
            Stop-Transcript | Out-Null
        }
        exit 1
    }

    Write-Host ""
    Write-ToolVersions $status
    if ($transcriptStarted) {
        Stop-Transcript | Out-Null
    }
}
catch {
    Write-Error $_
    Write-Host ("Install log saved to " + $LogPath)
    if ($PauseOnError) {
        Read-Host "Press Enter to close"
    }
    try {
        if ($transcriptStarted) {
            Stop-Transcript | Out-Null
        }
    } catch {
    }
    exit 1
}

param(
    [Parameter(Mandatory = $true)]
    [string]$ProjectDir,

    [switch]$EnableSccache
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $ProjectDir)) {
    throw "Windows build directory not found: $ProjectDir"
}

$steps = New-Object System.Collections.Generic.List[object]
$originalRustcWrapper = $env:RUSTC_WRAPPER
$sccachePath = $null
$bunBinDir = Join-Path $env:USERPROFILE ".bun\bin"
$bunFallback = Join-Path $bunBinDir "bun.exe"
$bunPath = $null

$bunCommand = Get-Command bun -ErrorAction SilentlyContinue
if ($bunCommand) {
    $bunPath = $bunCommand.Source
} elseif (Test-Path -LiteralPath $bunFallback) {
    $bunPath = $bunFallback
} else {
    throw "bun not found on PATH and %USERPROFILE%\\.bun\\bin\\bun.exe is missing"
}

if ($EnableSccache) {
    $sccacheCommand = Get-Command sccache -ErrorAction SilentlyContinue
    if (-not $sccacheCommand) {
        $sccacheCommand = Get-Command sccache.exe -ErrorAction SilentlyContinue
    }
    if (-not $sccacheCommand) {
        $wingetPackagesDir = Join-Path $env:LOCALAPPDATA "Microsoft\WinGet\Packages"
        if (Test-Path -LiteralPath $wingetPackagesDir) {
            $candidate = Get-ChildItem -LiteralPath $wingetPackagesDir -Filter "Mozilla.sccache*" -Directory -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending |
                Select-Object -First 1
            if ($candidate) {
                $sccacheExe = Get-ChildItem -LiteralPath $candidate.FullName -Filter "sccache.exe" -File -Recurse -ErrorAction SilentlyContinue |
                    Select-Object -First 1
                if ($sccacheExe) {
                    $sccacheCommand = [pscustomobject]@{ Source = $sccacheExe.FullName }
                }
            }
        }
    }

    if ($sccacheCommand) {
        $sccachePath = $sccacheCommand.Source
        $env:RUSTC_WRAPPER = $sccachePath
        try {
            & $sccachePath --zero-stats | Out-Null
        } catch {
            Write-Warning ("sccache stats reset failed: " + $_.Exception.Message)
        }
        Write-Host ("Using sccache via " + $sccachePath)
    } else {
        Write-Warning "TAURHAUS_WINDOWS_USE_SCCACHE=1 but sccache was not found on PATH."
    }
}

Push-Location -LiteralPath $ProjectDir
try {
    $start = Get-Date
    Write-Host "[windows_bun_install] starting..."
    & $bunPath install --frozen-lockfile
    $elapsed = (Get-Date) - $start
    $steps.Add([pscustomobject]@{ Name = "windows_bun_install"; Seconds = [Math]::Round($elapsed.TotalSeconds, 2) })

    $start = Get-Date
    Write-Host "[windows_cargo_tauri_build] starting..."
    $env:PATH = $bunBinDir + ";" + $env:PATH
    cargo tauri build --bundles nsis
    $elapsed = (Get-Date) - $start
    $steps.Add([pscustomobject]@{ Name = "windows_cargo_tauri_build"; Seconds = [Math]::Round($elapsed.TotalSeconds, 2) })
}
finally {
    Pop-Location
    if ($null -ne $originalRustcWrapper) {
        $env:RUSTC_WRAPPER = $originalRustcWrapper
    } else {
        Remove-Item Env:RUSTC_WRAPPER -ErrorAction SilentlyContinue
    }
}

Write-Host ""
Write-Host "Windows step summary:"
$steps | Format-Table -AutoSize

if ($EnableSccache -and $sccachePath) {
    Write-Host ""
    Write-Host "sccache stats:"
    & $sccachePath --show-stats
}

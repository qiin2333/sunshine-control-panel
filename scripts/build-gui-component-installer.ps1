param(
    [switch]$SkipBuild,
    [switch]$Beta
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$TauriRoot = Join-Path $ProjectRoot 'src-tauri'
$InstallerRoot = Join-Path $TauriRoot 'installer'
$StageDir = Join-Path $InstallerRoot 'staging'
$DistDir = Join-Path $InstallerRoot 'dist'
$NsiFile = Join-Path $InstallerRoot 'sunshine-gui-component.nsi'

function Find-MakeNsis {
    $cmd = Get-Command makensis.exe -ErrorAction SilentlyContinue
    $candidates = @(
        $(if ($cmd) { $cmd.Source }),
        "$env:ProgramFiles\NSIS\makensis.exe",
        "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
        'C:\msys64\ucrt64\bin\makensis.exe'
    ) | Where-Object { $_ -and (Test-Path $_) }

    return $candidates | Select-Object -First 1
}

function Resolve-GuiExe {
    $paths = @(
        (Join-Path $TauriRoot 'target\release\sunshine-gui.exe'),
        (Join-Path $TauriRoot 'target\x86_64-pc-windows-msvc\release\sunshine-gui.exe'),
        (Join-Path $TauriRoot 'target\x86_64-pc-windows-gnu\release\sunshine-gui.exe')
    )

    $candidates = $paths |
        Where-Object { Test-Path $_ } |
        ForEach-Object { Get-Item $_ } |
        Sort-Object LastWriteTime -Descending

    if ($candidates) {
        return $candidates[0].FullName
    }

    throw 'sunshine-gui.exe not found. Please build the GUI first.'
}

function Resolve-WebViewLoader([string]$GuiExe) {
    $dir = Split-Path -Parent $GuiExe
    $loader = Join-Path $dir 'WebView2Loader.dll'
    if (Test-Path $loader) {
        return $loader
    }
    return $null
}

Push-Location $ProjectRoot
try {
    if (-not $SkipBuild) {
        Write-Host '==> Building renderer assets' -ForegroundColor Cyan
        npm run build:renderer
        if ($LASTEXITCODE -ne 0) { throw 'build:renderer failed' }

        Write-Host '==> Building Tauri GUI (release)' -ForegroundColor Cyan
        $cargoArgs = @('build', '--release', '--manifest-path', (Join-Path $TauriRoot 'Cargo.toml'))
        if ($Beta) {
            $cargoArgs += @('--features', 'beta')
        }
        cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) { throw 'cargo build --release failed' }
    }

    $makensis = Find-MakeNsis
    if (-not $makensis) {
        throw 'makensis.exe from NSIS was not found'
    }

    $packageJson = Get-Content (Join-Path $ProjectRoot 'package.json') -Raw | ConvertFrom-Json
    $version = $packageJson.version

    $guiExe = Resolve-GuiExe
    $loader = Resolve-WebViewLoader $guiExe

    New-Item -ItemType Directory -Path $StageDir -Force | Out-Null
    New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

    Remove-Item (Join-Path $StageDir '*') -Force -Recurse -ErrorAction SilentlyContinue
    Copy-Item $guiExe (Join-Path $StageDir 'sunshine-gui.exe') -Force
    if ($loader) {
        Copy-Item $loader (Join-Path $StageDir 'WebView2Loader.dll') -Force
    }

    Write-Host '==> Building GUI component installer with NSIS' -ForegroundColor Cyan
    & $makensis '/INPUTCHARSET' 'UTF8' "/DVERSION=$version" "/DSOURCE_DIR=$StageDir" "/DOUTPUT_DIR=$DistDir" $NsiFile
    if ($LASTEXITCODE -ne 0) { throw 'NSIS GUI component installer build failed' }

    Write-Host ''
    Write-Host 'GUI component installer generated:' -ForegroundColor Green
    Get-ChildItem $DistDir -Filter 'Sunshine-GUI-Setup-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1 | ForEach-Object {
        Write-Host ('   ' + $_.FullName) -ForegroundColor Green
        Write-Host ('   SHA256: ' + (Get-FileHash $_.FullName -Algorithm SHA256).Hash) -ForegroundColor Green
    }

    # Staging is only an NSIS input cache. Keep failed-build staging for
    # diagnostics, but remove it after a successful package is produced.
    Remove-Item $StageDir -Force -Recurse
}
finally {
    Pop-Location
}

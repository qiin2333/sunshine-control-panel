param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$TauriRoot = Join-Path $ProjectRoot 'src-tauri'
$InstallerRoot = Join-Path $TauriRoot 'installer'
$StageDir = Join-Path $InstallerRoot 'staging'
$DistDir = Join-Path $InstallerRoot 'dist'
$IssFile = Join-Path $InstallerRoot 'sunshine-gui-overlay.iss'

function Find-Iscc {
    $cmd = Get-Command iscc.exe -ErrorAction SilentlyContinue
    $programFilesX86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')

    $candidates = @(
        $(if ($cmd) { $cmd.Source }),
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        $(if ($programFilesX86) { Join-Path $programFilesX86 'Inno Setup 6\ISCC.exe' })
    ) | Where-Object { $_ -and (Test-Path $_) }

    return $candidates | Select-Object -First 1
}

function Resolve-GuiExe {
    $paths = @(
        (Join-Path $TauriRoot 'target\release\sunshine-gui.exe'),
        (Join-Path $TauriRoot 'target\x86_64-pc-windows-msvc\release\sunshine-gui.exe'),
        (Join-Path $TauriRoot 'target\x86_64-pc-windows-gnu\release\sunshine-gui.exe')
    )

    foreach ($path in $paths) {
        if (Test-Path $path) {
            return $path
        }
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
        cargo build --release --manifest-path (Join-Path $TauriRoot 'Cargo.toml')
        if ($LASTEXITCODE -ne 0) { throw 'cargo build --release failed' }
    }

    $iscc = Find-Iscc
    if (-not $iscc) {
        throw 'ISCC.exe from Inno Setup 6 was not found'
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

    Write-Host '==> Building GUI overlay installer' -ForegroundColor Cyan
    & $iscc "/DMyAppVersion=$version" "/DSourceDir=$StageDir" "/DOutputDir=$DistDir" $IssFile
    if ($LASTEXITCODE -ne 0) { throw 'ISCC build failed' }

    Write-Host ''
    Write-Host 'GUI overlay installer generated:' -ForegroundColor Green
    Get-ChildItem $DistDir -Filter 'Sunshine-GUI-Overlay-*.exe' | Sort-Object LastWriteTime -Descending | Select-Object -First 1 | ForEach-Object {
        Write-Host ('   ' + $_.FullName) -ForegroundColor Green
    }
}
finally {
    Pop-Location
}

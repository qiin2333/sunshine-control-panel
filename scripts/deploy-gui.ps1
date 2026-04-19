# deploy-gui.ps1 — 编译并部署 GUI 到 Sunshine 安装目录
# 用法: .\scripts\deploy-gui.ps1 [-Release] [-NoBuild]
param(
    [switch]$Release,
    [switch]$NoBuild
)

$ErrorActionPreference = "Stop"

# 定位 Sunshine 安装目录
$sunshineDir = $null
$candidates = @(
    "C:\Program Files\Sunshine",
    "C:\Program Files (x86)\Sunshine",
    "${env:ProgramFiles}\Sunshine"
)
foreach ($dir in $candidates) {
    if (Test-Path "$dir\sunshine.exe") {
        $sunshineDir = $dir
        break
    }
}

if (-not $sunshineDir) {
    # 尝试从注册表读取
    try {
        $regPath = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Sunshine_is1"
        if (Test-Path $regPath) {
            $sunshineDir = (Get-ItemProperty $regPath).InstallLocation.TrimEnd('\')
        }
    } catch {}
}

if (-not $sunshineDir) {
    Write-Host "❌ 找不到 Sunshine 安装目录" -ForegroundColor Red
    exit 1
}

$guiDir = Join-Path $sunshineDir "assets\gui"
Write-Host "📁 Sunshine 目录: $sunshineDir" -ForegroundColor Cyan
Write-Host "📁 GUI 目标目录: $guiDir" -ForegroundColor Cyan

# 编译
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
# 如果是从 sunshine-control-panel/scripts 运行，projectRoot 就是 sunshine-control-panel
if (-not (Test-Path "$projectRoot\src-tauri\Cargo.toml")) {
    $projectRoot = Split-Path -Parent $PSScriptRoot
}
if (-not (Test-Path "$projectRoot\src-tauri\Cargo.toml")) {
    # 直接定位到 sunshine-control-panel
    $projectRoot = $PSScriptRoot | Split-Path -Parent
}

if (-not $NoBuild) {
    Write-Host "`n🔨 编译 Sunshine GUI..." -ForegroundColor Yellow
    Push-Location "$projectRoot\src-tauri"

    if ($Release) {
        cargo build --release
    } else {
        cargo build
    }

    if ($LASTEXITCODE -ne 0) {
        Pop-Location
        Write-Host "❌ 编译失败" -ForegroundColor Red
        exit 1
    }
    Pop-Location
}

# 定位编译产物
$profile = if ($Release) { "release" } else { "debug" }
$exePaths = @(
    "$projectRoot\src-tauri\target\$profile\sunshine-gui.exe",
    "$projectRoot\src-tauri\target\x86_64-pc-windows-msvc\$profile\sunshine-gui.exe"
)

$exeSrc = $null
foreach ($p in $exePaths) {
    if (Test-Path $p) {
        $exeSrc = $p
        break
    }
}

if (-not $exeSrc) {
    Write-Host "❌ 找不到编译产物 sunshine-gui.exe" -ForegroundColor Red
    exit 1
}

Write-Host "📦 编译产物: $exeSrc" -ForegroundColor Green
$exeSize = [math]::Round((Get-Item $exeSrc).Length / 1MB, 1)
Write-Host "   大小: ${exeSize} MB"

# 部署
if (-not (Test-Path $guiDir)) {
    New-Item -ItemType Directory -Path $guiDir -Force | Out-Null
}

# 先停止正在运行的 GUI
$running = Get-Process -Name "sunshine-gui" -ErrorAction SilentlyContinue
if ($running) {
    Write-Host "⏹ 停止运行中的 sunshine-gui..." -ForegroundColor Yellow
    $running | Stop-Process -Force
    Start-Sleep -Seconds 1
}

Write-Host "📋 复制到 $guiDir ..." -ForegroundColor Yellow
Copy-Item $exeSrc "$guiDir\sunshine-gui.exe" -Force

# WebView2Loader.dll (如果存在)
$loaderPath = Split-Path $exeSrc -Parent | Join-Path -ChildPath "WebView2Loader.dll"
if (Test-Path $loaderPath) {
    Copy-Item $loaderPath "$guiDir\WebView2Loader.dll" -Force
    Write-Host "   + WebView2Loader.dll"
}

Write-Host "`n✅ GUI 已部署到 $guiDir" -ForegroundColor Green
Write-Host "   sunshine-gui.exe ($exeSize MB)" -ForegroundColor Green

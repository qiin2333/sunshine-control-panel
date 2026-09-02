param(
    [ValidateSet('Debug', 'Release')]
    [string]$Configuration = 'Release',
    [string]$Target = ''
)

$ErrorActionPreference = 'Stop'

$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$WorkspaceManifest = Join-Path $ProjectRoot 'native-plugins\Cargo.toml'
$Profile = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$CargoArguments = @(
    'build',
    '--manifest-path', $WorkspaceManifest,
    '-p', 'alkaidlab-plugin-stylus'
)
if ($Configuration -eq 'Release') {
    $CargoArguments += '--release'
}
if ($Target) {
    $CargoArguments += @('--target', $Target)
}

Write-Host '==> Building native tool plugins' -ForegroundColor Cyan
cargo @CargoArguments
if ($LASTEXITCODE -ne 0) {
    throw 'Native tool plugin build failed'
}

$PluginTargetRoot = Join-Path $ProjectRoot 'native-plugins\target'
$PluginSource = if ($Target) {
    Join-Path $PluginTargetRoot "$Target\$Profile\alkaidlab_plugin_stylus.dll"
} else {
    Join-Path $PluginTargetRoot "$Profile\alkaidlab_plugin_stylus.dll"
}
if (-not (Test-Path -LiteralPath $PluginSource)) {
    throw "Native tool plugin output was not found: $PluginSource"
}

$GuiTargetRoot = Join-Path $ProjectRoot 'src-tauri\target'
$GuiOutput = if ($Target) {
    Join-Path $GuiTargetRoot "$Target\$Profile"
} else {
    Join-Path $GuiTargetRoot $Profile
}
New-Item -ItemType Directory -Path $GuiOutput -Force | Out-Null

$Destination = Join-Path $GuiOutput 'alkaidlab-plugin-stylus.dll'
Copy-Item -LiteralPath $PluginSource -Destination $Destination -Force
Write-Host "Native plugin: $Destination" -ForegroundColor Green

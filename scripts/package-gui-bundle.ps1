param(
    [Parameter(Mandatory = $true)]
    [string]$SourceDirectory,
    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'

$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
$output = [System.IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $output
$requiredFiles = @(
    'sunshine-gui.exe',
    'alkaidlab-plugin-stylus.dll'
)

foreach ($name in $requiredFiles) {
    $path = Join-Path $source $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required GUI bundle file was not found: $name"
    }
    if ((Get-Item -LiteralPath $path).Length -eq 0) {
        throw "Required GUI bundle file is empty: $name"
    }
}

New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("sunshine-gui-bundle-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stage -Force | Out-Null

try {
    foreach ($name in $requiredFiles) {
        Copy-Item -LiteralPath (Join-Path $source $name) -Destination (Join-Path $stage $name)
    }
    $webViewLoader = Join-Path $source 'WebView2Loader.dll'
    if (Test-Path -LiteralPath $webViewLoader -PathType Leaf) {
        Copy-Item -LiteralPath $webViewLoader -Destination (Join-Path $stage 'WebView2Loader.dll')
    }

    if (Test-Path -LiteralPath $output) {
        Remove-Item -LiteralPath $output -Force
    }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $output -CompressionLevel Optimal
    if (-not (Test-Path -LiteralPath $output -PathType Leaf) -or
        (Get-Item -LiteralPath $output).Length -eq 0) {
        throw 'GUI bundle archive was not created'
    }
    Write-Host "GUI bundle: $output" -ForegroundColor Green
}
finally {
    if (Test-Path -LiteralPath $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}

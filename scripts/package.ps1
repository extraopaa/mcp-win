param([string]$OutputDirectory = 'dist')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    & (Join-Path $PSScriptRoot 'fetch-engines.ps1')
    & (Join-Path $PSScriptRoot 'fetch-cloudflared.ps1')
    & (Join-Path $PSScriptRoot 'fetch-duckdb.ps1')
    if (-not (Test-Path -LiteralPath 'vendor\frp\frp_0.71.1_windows_amd64\frpc.exe')) {
        & (Join-Path $PSScriptRoot 'fetch-frp.ps1')
    }
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    & $cargoPath build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Compilation échouée' }
    & (Join-Path $PSScriptRoot 'licenses.ps1')
    $dist = if ([IO.Path]::IsPathRooted($OutputDirectory)) { $OutputDirectory } else { Join-Path $projectRoot $OutputDirectory }
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    for ($attempt = 1; $attempt -le 5; $attempt++) {
        try { Copy-Item -LiteralPath 'target\release\mcp-win.exe' -Destination (Join-Path $dist 'MCP Win.exe'); break }
        catch { if ($attempt -eq 5) { throw }; Start-Sleep -Seconds 1 }
    }
    Copy-Item -LiteralPath 'README.md','CHANGELOG.md','LICENSE','THIRD-PARTY-NOTICES.md','VALIDATION.md' -Destination $dist
    New-Item -ItemType Directory -Force (Join-Path $dist 'assets/brand') | Out-Null
    Copy-Item -LiteralPath 'assets/brand/icon-original.png' -Destination (Join-Path $dist 'assets/brand')
    Copy-Item -LiteralPath 'third-party-licenses','relay' -Destination $dist -Recurse -Force
    Copy-Item -LiteralPath 'docs' -Destination $dist -Recurse -Force
    Copy-Item -LiteralPath 'vendor\frp\frp_0.71.1_windows_amd64\LICENSE' -Destination (Join-Path $dist 'LICENSE-frp.txt')
    Copy-Item -LiteralPath 'assets\fonts\OFL.txt' -Destination (Join-Path $dist 'LICENSE-fonts.txt')
    New-Item -ItemType Directory -Force (Join-Path $dist 'engines') | Out-Null
    Copy-Item -LiteralPath 'vendor/engines/runtime/duckdb' -Destination (Join-Path $dist 'engines') -Recurse -Force
    Copy-Item -LiteralPath 'vendor/duckdb/LICENSE' -Destination (Join-Path $dist 'LICENSE-duckdb.txt')
    Copy-Item -LiteralPath 'vendor/engines/runtime/cloudflared.exe' -Destination (Join-Path $dist 'engines')
    Copy-Item -LiteralPath 'vendor/cloudflared/LICENSE' -Destination (Join-Path $dist 'LICENSE-cloudflared.txt')
    Copy-Item -LiteralPath 'vendor/engines/runtime/typst.exe','vendor/engines/runtime/pdfium.dll','vendor/engines/runtime/tesseract' -Destination (Join-Path $dist 'engines') -Recurse -Force
    Copy-Item -LiteralPath 'vendor/engines/typst/typst-x86_64-pc-windows-msvc/LICENSE' -Destination (Join-Path $dist 'LICENSE-typst.txt')
    Copy-Item -LiteralPath 'vendor/engines/typst/typst-x86_64-pc-windows-msvc/NOTICE' -Destination (Join-Path $dist 'NOTICE-typst.txt')
    Copy-Item -LiteralPath 'vendor/engines/pdfium/licenses' -Destination (Join-Path $dist 'licenses-pdfium') -Recurse -Force
    Copy-Item -LiteralPath 'vendor/engines/pdfium/LICENSE' -Destination (Join-Path $dist 'LICENSE-pdfium.txt')
    Copy-Item -LiteralPath 'assets/OCR-MODELS-NOTICE.md' -Destination $dist
    Write-Output "Application prête : $dist\MCP Win.exe"
} finally { Pop-Location }

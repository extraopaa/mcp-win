$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not (Test-Path -LiteralPath 'vendor\frp\frp_0.71.1_windows_amd64\frpc.exe')) {
        & (Join-Path $PSScriptRoot 'fetch-frp.ps1')
    }
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    & $cargoPath build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Compilation échouée' }
    & (Join-Path $PSScriptRoot 'licenses.ps1')
    $dist = Join-Path $projectRoot 'dist'
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    Copy-Item -LiteralPath 'target\release\mcp-win.exe' -Destination (Join-Path $dist 'MCP Win.exe')
    Copy-Item -LiteralPath 'README.md','LICENSE','THIRD-PARTY-NOTICES.md','VALIDATION.md' -Destination $dist
    Copy-Item -LiteralPath 'third-party-licenses','relay' -Destination $dist -Recurse -Force
    Copy-Item -LiteralPath 'docs' -Destination $dist -Recurse -Force
    Copy-Item -LiteralPath 'vendor\frp\frp_0.71.1_windows_amd64\LICENSE' -Destination (Join-Path $dist 'LICENSE-frp.txt')
    Copy-Item -LiteralPath 'assets\fonts\OFL.txt' -Destination (Join-Path $dist 'LICENSE-fonts.txt')
    Write-Output "Application prête : $dist\MCP Win.exe"
} finally { Pop-Location }

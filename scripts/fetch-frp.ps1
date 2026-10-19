$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$destination = Join-Path $projectRoot 'vendor\frp'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
$zipPath = Join-Path $destination 'frp.zip'
Invoke-WebRequest 'https://github.com/fatedier/frp/releases/download/v0.71.1/frp_0.71.1_windows_amd64.zip' -OutFile $zipPath
$actual = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash
if ($actual -ne 'B90074B2B6A1D792A7A40F085AC6D276BCAB2D1BC60201F8A86CA53CE65CE821') { throw 'SHA-256 frp invalide' }
Expand-Archive -LiteralPath $zipPath -DestinationPath $destination -Force
Write-Output 'frpc 0.71.1 téléchargé et vérifié.'

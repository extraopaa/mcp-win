param()
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$runtime = Join-Path $projectRoot 'vendor/engines/runtime'
$target = Join-Path $runtime 'cloudflared.exe'
$expected = '4D2490E74082C5D48DBFEDAF92576FA5BD1702379AB88EB7FAA0CFBE8DC891B6'
New-Item -ItemType Directory -Force $runtime | Out-Null
if (-not (Test-Path -LiteralPath $target) -or (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $expected) {
 $download = Join-Path $runtime 'cloudflared.download'
 Invoke-WebRequest -UseBasicParsing 'https://github.com/cloudflare/cloudflared/releases/download/2026.10.1/cloudflared-windows-amd64.exe' -OutFile $download
 if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne $expected) { throw 'Empreinte cloudflared incorrecte : binaire non installé.' }
 Move-Item -LiteralPath $download -Destination $target -Force
}
$licenses = Join-Path $projectRoot 'vendor/cloudflared'
New-Item -ItemType Directory -Force $licenses | Out-Null
if (-not (Test-Path -LiteralPath (Join-Path $licenses 'LICENSE'))) {
 Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/cloudflare/cloudflared/2026.10.1/LICENSE' -OutFile (Join-Path $licenses 'LICENSE')
}
Write-Output 'cloudflared 2026.10.1 prêt, empreinte SHA-256 vérifiée.'

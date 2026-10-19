param()
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$dest=Join-Path $root 'vendor/engines/runtime/duckdb'
$zip=Join-Path $root 'vendor/engines/duckdb-v1.5.6.zip'
$expected='798EAE475D07C645FF3B914F7B0E676D06502B5412DD7552638FAFED81F9E916'
New-Item -ItemType Directory -Force $dest | Out-Null
if (-not (Test-Path -LiteralPath $zip) -or (Get-FileHash -LiteralPath $zip).Hash -ne $expected) {
 Invoke-WebRequest -UseBasicParsing 'https://github.com/duckdb/duckdb/releases/download/v1.5.6/duckdb_cli-windows-amd64.zip' -OutFile $zip
}
if ((Get-FileHash -LiteralPath $zip).Hash -ne $expected) {throw 'Empreinte archive DuckDB incorrecte'}
Expand-Archive -LiteralPath $zip -DestinationPath $dest -Force
$license=Join-Path $root 'vendor/duckdb'
New-Item -ItemType Directory -Force $license | Out-Null
if (-not (Test-Path -LiteralPath (Join-Path $license 'LICENSE'))) {Invoke-WebRequest -UseBasicParsing 'https://raw.githubusercontent.com/duckdb/duckdb/v1.5.6/LICENSE' -OutFile (Join-Path $license 'LICENSE')}
Get-FileHash -LiteralPath (Join-Path $dest 'duckdb.exe') | Select-Object Hash

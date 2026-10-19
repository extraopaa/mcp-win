param([string]$Version = '0.1.0')
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+([-.][A-Za-z0-9.-]+)?$') { throw 'Version invalide' }
Add-Type -AssemblyName System.IO.Compression
$projectRoot = Split-Path -Parent $PSScriptRoot
$source = Join-Path $projectRoot 'dist'
$output = Join-Path $projectRoot 'release-assets'
New-Item -ItemType Directory -Path $output -Force | Out-Null
$filename = "mcp-win-v$Version-windows-x64.zip"
$archivePath = Join-Path $output $filename
$stream = [IO.File]::Open($archivePath, [IO.FileMode]::Create)
$archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($file in (Get-ChildItem -LiteralPath $source -Recurse -File | Sort-Object FullName)) {
        $name = $file.FullName.Substring($source.Length).TrimStart('\','/').Replace('\','/')
        $entry = $archive.CreateEntry($name, [IO.Compression.CompressionLevel]::Optimal)
        $inputStream = [IO.File]::OpenRead($file.FullName)
        $entryStream = $entry.Open()
        try { $inputStream.CopyTo($entryStream) } finally { $entryStream.Dispose(); $inputStream.Dispose() }
    }
} finally { $archive.Dispose(); $stream.Dispose() }
$hash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLower()
"$hash  $filename" | Set-Content -Encoding ascii (Join-Path $output 'SHA256SUMS.txt')
Write-Output $archivePath

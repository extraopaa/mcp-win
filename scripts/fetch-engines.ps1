$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = Join-Path (Split-Path -Parent $PSScriptRoot) 'vendor/engines'
New-Item -ItemType Directory -Force $root | Out-Null
function Fetch($url, $name, $hash) {
    $path = Join-Path $root $name
    if (-not (Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path).Hash -ne $hash) {
        Invoke-WebRequest -Uri $url -OutFile $path
    }
    if ((Get-FileHash -LiteralPath $path).Hash -ne $hash) { throw "Empreinte incorrecte : $name" }
}
Fetch 'https://github.com/typst/typst/releases/download/v0.15.1/typst-x86_64-pc-windows-msvc.zip' 'typst.zip' '19CE3551153C2FE7EE9FA2F95208310C8F4D3209FEDB699E0333FAF8913F6736'
Fetch 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8100/pdfium-win-x64.tgz' 'pdfium.tgz' '51621BCC1D9BE9EA448F4B84B0516862B928E3968D3FCA899AF1A6D6E2848D6D'
Fetch 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/4.1.0/fra.traineddata' 'fra.traineddata' 'CED037562E8C80C13122DECE28DD477D399AF80911A28791A66A63AC1E3445CA'
Fetch 'https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/4.1.0/eng.traineddata' 'eng.traineddata' '7D4322BD2A7749724879683FC3912CB542F19906C83BCC1A52132556427170B2'
Expand-Archive -LiteralPath (Join-Path $root 'typst.zip') -DestinationPath (Join-Path $root 'typst') -Force
New-Item -ItemType Directory -Force (Join-Path $root 'pdfium'),(Join-Path $root 'runtime') | Out-Null
& tar -xzf (Join-Path $root 'pdfium.tgz') -C (Join-Path $root 'pdfium')
if ($LASTEXITCODE -ne 0) { throw 'Extraction Pdfium échouée' }
foreach ($source in @('typst/typst-x86_64-pc-windows-msvc/typst.exe','pdfium/bin/pdfium.dll')) {
    $from = Join-Path $root $source
    $to = Join-Path (Join-Path $root 'runtime') (Split-Path -Leaf $source)
    if (-not (Test-Path -LiteralPath $to) -or (Get-FileHash -LiteralPath $from).Hash -ne (Get-FileHash -LiteralPath $to).Hash) {
        Copy-Item -LiteralPath $from -Destination $to -Force
    }
}
& (Join-Path $PSScriptRoot 'build-tesseract.ps1')
$data = Join-Path $root 'runtime/tesseract/tessdata'
New-Item -ItemType Directory -Force $data | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'fra.traineddata'),(Join-Path $root 'eng.traineddata') -Destination $data -Force
Write-Output 'Typst, Pdfium, Tesseract et modèles français/anglais prêts.'

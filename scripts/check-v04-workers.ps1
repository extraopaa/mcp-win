$ErrorActionPreference='Stop'
$qaBase=Join-Path (Get-Location) 'target/qa-v04'
$qaRoot=Join-Path $qaBase ('workers-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $qaRoot | Out-Null
Copy-Item -LiteralPath (Join-Path $qaBase 'filled-form.pdf') -Destination (Join-Path $qaRoot 'filled-form.pdf')
$workerDir=Join-Path $qaRoot 'worker-app'
New-Item -ItemType Directory -Force (Join-Path $workerDir 'engines') | Out-Null
Copy-Item -LiteralPath 'target/release/mcp-win.exe' -Destination (Join-Path $workerDir 'mcp-win.exe') -Force
Copy-Item -LiteralPath 'vendor/engines/runtime/pdfium.dll' -Destination (Join-Path $workerDir 'engines/pdfium.dll') -Force
function Run-SyntheticWorker($operation) {
 $requestPath=Join-Path $qaRoot 'worker-request.json'
 $outputPath=Join-Path $qaRoot 'worker-result.json'
 $request=@{folders=@(@{id='qa';path=$qaRoot;writable=$true});operation=$operation}
 [IO.File]::WriteAllText($requestPath,($request | ConvertTo-Json -Depth 20),[Text.UTF8Encoding]::new($false))
 $process=Start-Process -FilePath (Join-Path $workerDir 'mcp-win.exe') -ArgumentList '--mcp-worker' -RedirectStandardInput $requestPath -RedirectStandardOutput $outputPath -WindowStyle Hidden -Wait -PassThru
 if ($process.ExitCode -ne 0) {throw 'Le worker de production a échoué'}
 $result=Get-Content -LiteralPath $outputPath -Raw -Encoding UTF8 | ConvertFrom-Json
 if ($result.PSObject.Properties.Name -contains 'Err') {throw $result.Err}
 return $result.Ok
}
$fields=Run-SyntheticWorker @{InspectForm=@{folder='qa';path='filled-form.pdf';offset=0;limit=50}}
if ($fields.total -ne 3) {throw 'Champs AcroForm non retrouvés'}
$filled=Run-SyntheticWorker @{FillForm=@{folder='qa';path='filled-form.pdf';output_path='worker-filled.pdf';expected_sha256=(Get-FileHash -LiteralPath (Join-Path $qaRoot 'filled-form.pdf')).Hash.ToLowerInvariant();values=@{'client.name'='Clémence Été';approved=$false;department='Finance'}}}
$flat=Run-SyntheticWorker @{CombinePdf=@{folder='qa';inputs=@(@{path='worker-filled.pdf';pages='1'});output_path='worker-flat.pdf';flatten=$true}}
$archive=Run-SyntheticWorker @{Archive=@{Create=@{folder='qa';paths=@('worker-filled.pdf','worker-flat.pdf');output_path='worker-delivery.7z'}}}
$archiveList=Run-SyntheticWorker @{Archive=@{Inspect=@{folder='qa';path='worker-delivery.7z';offset=0;limit=20}}}
if ($archiveList.total -ne 2) {throw 'Archive générée incomplète'}
Write-Output 'Workers de production : inspection/remplissage AcroForm, PDF aplati, création/inspection 7z réussis sur fixtures locales.'

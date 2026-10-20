param([string]$PackageDirectory='dist-v1')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $cargo=Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    & $cargo build --locked --example update-fixture --example sign-update
    if($LASTEXITCODE -ne 0){throw 'Compilation des fixtures échouée'}
    $fixture=Join-Path $projectRoot ('target\qa-updater\'+[guid]::NewGuid().ToString('N'))
    $stage=Join-Path $fixture 'stage'
    $target=Join-Path $fixture 'app'
    New-Item -ItemType Directory -Force -Path (Join-Path $stage 'engines'),$target | Out-Null
    Copy-Item -LiteralPath 'target\debug\examples\update-fixture.exe' -Destination (Join-Path $stage 'MCP Win.exe')
    [IO.File]::WriteAllText((Join-Path $stage 'engines\typst.exe'),'synthetic-engine')
    [IO.File]::WriteAllText((Join-Path $stage 'engines\pdfium.dll'),'synthetic-engine')
    [IO.File]::WriteAllText((Join-Path $target 'settings.json'),'PRESERVE-USER-SETTINGS')
    [IO.File]::WriteAllText((Join-Path $target 'MCP Win.exe'),'OLD-SYNTHETIC-EXE')
    Add-Type -AssemblyName System.IO.Compression
    $archive=Join-Path $fixture 'package.zip'
    $stream=[IO.File]::Open($archive,[IO.FileMode]::Create)
    $zip=[IO.Compression.ZipArchive]::new($stream,[IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach($file in Get-ChildItem -LiteralPath $stage -Recurse -File){
            $name=$file.FullName.Substring($stage.Length).TrimStart('\').Replace('\','/')
            $entry=$zip.CreateEntry($name)
            $inputStream=[IO.File]::OpenRead($file.FullName)
            $outputStream=$entry.Open()
            try{$inputStream.CopyTo($outputStream)}finally{$inputStream.Dispose();$outputStream.Dispose()}
        }
    }finally{$zip.Dispose();$stream.Dispose()}
    $manifest=@{format=1;version='2.0.0';target='windows-x86_64';url='https://github.com/extraopaa/mcp-win/releases/download/v2.0.0/mcp-win-v2.0.0-windows-x64.zip';sha256=(Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLower();size=(Get-Item -LiteralPath $archive).Length}
    $manifestPath=Join-Path $fixture 'update.json'
    [IO.File]::WriteAllText($manifestPath,($manifest|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    & 'target\debug\examples\sign-update.exe' sign (Join-Path $env:USERPROFILE '.mcp-win-release\updater.key') $manifestPath ($manifestPath+'.sig')
    if($LASTEXITCODE -ne 0){throw 'Signature de fixture échouée'}
    [IO.File]::WriteAllText((Join-Path $fixture 'plan.json'),(@{target=$target;executable='MCP Win.exe';parent=0}|ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    $package=if([IO.Path]::IsPathRooted($PackageDirectory)){$PackageDirectory}else{Join-Path $projectRoot $PackageDirectory}
    $helper=Start-Process -FilePath (Join-Path $package 'MCP Win.exe') -ArgumentList @('--apply-update',('"'+$fixture+'"')) -WindowStyle Hidden -PassThru
    if(-not $helper.WaitForExit(30000)){Stop-Process -Id $helper.Id;throw 'Installation synthétique bloquée'}
    for($i=0;$i -lt 50 -and -not(Test-Path -LiteralPath (Join-Path $target 'updated.marker'));$i++){Start-Sleep -Milliseconds 100}
    if(-not(Test-Path -LiteralPath (Join-Path $target 'updated.marker'))){throw 'La fixture mise à jour ne s’est pas relancée'}
    if([IO.File]::ReadAllText((Join-Path $target 'settings.json')) -ne 'PRESERVE-USER-SETTINGS'){throw 'Préférences synthétiques altérées'}
    Write-Output 'Updater natif : manifeste signé, vérification du ZIP, remplacement portable, préférences conservées et relance validés. Aucun téléchargement ni publication.'
}finally{Pop-Location}

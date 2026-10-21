param([string]$PackageDirectory = 'dist-v101', [ValidateSet('ui-v1-native.cjs','office-native-smoke.cjs')][string]$ValidationScript = 'ui-v1-native.cjs')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('mcp-win.exe','MCP Win.exe') }) { throw 'Fermer MCP Win avant ce test de développement' }
$settingsPath = Join-Path $env:APPDATA 'org.mcpwin.desktop\settings.json'
$existing = Test-Path -LiteralPath $settingsPath
$previousBytes = if ($existing) { [IO.File]::ReadAllBytes($settingsPath) } else { $null }
$previousDebug = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$fixture = Join-Path $env:TEMP ('mcp-win-ui-' + [guid]::NewGuid().ToString('N'))
$appProcess = $null
$privatePath = [IO.Path]::GetFullPath((Join-Path $env:APPDATA 'org.mcpwin.desktop\.mcp-private'))
$previousPrivateFiles = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$privateBackups = @{}
if (Test-Path -LiteralPath $privatePath) {
    Get-ChildItem -LiteralPath $privatePath -Recurse -File | ForEach-Object { [void]$previousPrivateFiles.Add($_.FullName) }
}
foreach($name in @('brands.json','oauth-clients.json')) {
    $path=Join-Path $privatePath $name
    if(Test-Path -LiteralPath $path){$privateBackups[$name]=[IO.File]::ReadAllBytes($path)}
}
try {
    New-Item -ItemType Directory -Path $privatePath -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $privatePath 'brands.json'),'{"selected":"default","profiles":[{"id":"default","name":"Validation"}]}',[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $privatePath 'oauth-clients.json'),'[]',[Text.UTF8Encoding]::new($false))
    New-Item -ItemType Directory -Path $fixture -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $fixture 'note.txt'), 'SYNTHETIC-V1')
    if ($ValidationScript -eq 'office-native-smoke.cjs') {
        foreach($item in @(@('word.docx','office.docx'),@('grid.xlsx','office.xlsx'),@('deck.pptx','office.pptx'))) {
            Copy-Item -LiteralPath (Join-Path $projectRoot ('target/qa-office/'+$item[0])) -Destination (Join-Path $fixture $item[1])
        }
    }
    if ($ValidationScript -eq 'office-native-smoke.cjs') { Copy-Item -LiteralPath (Join-Path $projectRoot 'target/qa-office/sample.pdf') -Destination (Join-Path $fixture 'office.pdf') }
    $preferences = @{
        connection = @{mode='local';require_auth=$true;port=8787;token=[guid]::NewGuid().ToString('N');relay='';frp_server='';frp_port=7000;frp_token='';public_url=''}
        folders = @(@{id='ui-test';path=$fixture})
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $settingsPath) -Force | Out-Null
    [IO.File]::WriteAllText($settingsPath, ($preferences | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223'
    $package = if ([IO.Path]::IsPathRooted($PackageDirectory)) { $PackageDirectory } else { Join-Path $projectRoot $PackageDirectory }
    $appProcess = Start-Process -FilePath (Join-Path $package 'MCP Win.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
    & node (Join-Path $PSScriptRoot $ValidationScript)
    if ($LASTEXITCODE -ne 0) { throw 'Le test natif v1 a échoué' }
} finally {
    if ($appProcess) { $appProcess.Refresh(); if (-not $appProcess.HasExited) { Stop-Process -Id $appProcess.Id } }
    foreach($name in $privateBackups.Keys){[IO.File]::WriteAllBytes((Join-Path $privatePath $name),$privateBackups[$name])}
    # Remove only newly created synthetic test files; preserve all pre-existing state.
    $expectedPrivate = [IO.Path]::GetFullPath((Join-Path $env:APPDATA 'org.mcpwin.desktop\.mcp-private'))
    if ($privatePath -ne $expectedPrivate) { throw 'Chemin de nettoyage privé inattendu' }
    if (Test-Path -LiteralPath $privatePath) {
        Get-ChildItem -LiteralPath $privatePath -Recurse -File | ForEach-Object {
            $candidate = [IO.Path]::GetFullPath($_.FullName)
            if (-not $candidate.StartsWith($privatePath.TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Fichier hors dossier privé' }
            if (-not $previousPrivateFiles.Contains($candidate)) { Remove-Item -LiteralPath $candidate }
        }
    }
    if ($existing) { [IO.File]::WriteAllBytes($settingsPath, $previousBytes) } elseif (Test-Path -LiteralPath $settingsPath) { Remove-Item -LiteralPath $settingsPath }
    if ($null -eq $previousDebug) { Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue } else { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$previousDebug }
    $resolvedFixture=[IO.Path]::GetFullPath($fixture)
    $allowedTemp=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\'
    if (-not $resolvedFixture.StartsWith($allowedTemp,[StringComparison]::OrdinalIgnoreCase) -or -not (Split-Path -Leaf $resolvedFixture).StartsWith('mcp-win-ui-')) {throw 'Nettoyage refusé : chemin fixture inattendu'}
    if (Test-Path -LiteralPath $resolvedFixture) {Remove-Item -LiteralPath $resolvedFixture -Recurse -Force}
}

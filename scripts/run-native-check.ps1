param([string]$PackageDirectory = 'dist')
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
if (Get-CimInstance Win32_Process | Where-Object { $_.Name -in @('mcp-win.exe','MCP Win.exe') }) { throw 'Fermer MCP Win avant ce test de développement' }
$settingsPath = Join-Path $env:APPDATA 'org.mcpwin.desktop\settings.json'
$existing = Test-Path -LiteralPath $settingsPath
$previousBytes = if ($existing) { [IO.File]::ReadAllBytes($settingsPath) } else { $null }
$previousDebug = $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS
$fixture = Join-Path $env:TEMP ('mcp-win-ui-' + [guid]::NewGuid().ToString('N'))
$appProcess = $null
try {
    New-Item -ItemType Directory -Path $fixture -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $fixture 'note.txt'), 'NATIVE-TAURI-PUBLIC-OK')
    $preferences = @{
        connection = @{ mode='localtunnel'; port=8787; token=[guid]::NewGuid().ToString('N'); relay='https://localtunnel.me'; frp_server=''; frp_port=7000; frp_token=''; public_url='' }
        folders = @(@{id='ui-test';path=$fixture})
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $settingsPath) -Force | Out-Null
    [IO.File]::WriteAllText($settingsPath, ($preferences | ConvertTo-Json -Depth 6), [Text.UTF8Encoding]::new($false))
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223'
    $package = if ([IO.Path]::IsPathRooted($PackageDirectory)) { $PackageDirectory } else { Join-Path $projectRoot $PackageDirectory }
    $appProcess = Start-Process -FilePath (Join-Path $package 'MCP Win.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
    & node (Join-Path $PSScriptRoot 'ui-public-smoke.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Le test de connexion publique depuis Tauri a échoué' }
    & node (Join-Path $PSScriptRoot 'ui-smoke.cjs')
    if ($LASTEXITCODE -ne 0) { throw 'Le test de l’interface a échoué' }
} finally {
    if ($appProcess) { $appProcess.Refresh(); if (-not $appProcess.HasExited) { Stop-Process -Id $appProcess.Id } }
    if ($existing) { [IO.File]::WriteAllBytes($settingsPath, $previousBytes) } elseif (Test-Path -LiteralPath $settingsPath) { Remove-Item -LiteralPath $settingsPath }
    if ($null -eq $previousDebug) { Remove-Item Env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue } else { $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=$previousDebug }
    foreach ($name in @('note.txt','native.pdf','native.docx','native.pptx','native.xlsx','designed.pdf','scan.png','styled.docx')) { Remove-Item -LiteralPath (Join-Path $fixture $name) -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $fixture -ErrorAction SilentlyContinue
}

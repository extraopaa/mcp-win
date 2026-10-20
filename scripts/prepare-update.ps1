param(
    [string]$Version = '',
    [string]$SourceDirectory = 'dist',
    [string]$PrivateKey = (Join-Path $env:USERPROFILE '.mcp-win-release\updater.key')
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $packageVersion = (Select-String -LiteralPath Cargo.toml -Pattern '^version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value
    if (-not $Version) { $Version = $packageVersion }
    if ($Version -notmatch '^\d+\.\d+\.\d+$' -or $Version -ne $packageVersion) { throw 'La version doit être stable et correspondre à Cargo.toml.' }
    if (-not (Test-Path -LiteralPath $PrivateKey)) { throw 'Clé de signature absente. Voir docs/UPDATES.md.' }
    $cargoPath = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    & (Join-Path $PSScriptRoot 'archive-release.ps1') -Version $Version -SourceDirectory $SourceDirectory
    $filename = "mcp-win-v$Version-windows-x64.zip"
    $archivePath = Join-Path $projectRoot "release-assets\$filename"
    $manifest = [ordered]@{
        format = 1
        version = $Version
        target = 'windows-x86_64'
        url = "https://github.com/extraopaa/mcp-win/releases/download/v$Version/$filename"
        sha256 = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLower()
        size = (Get-Item -LiteralPath $archivePath).Length
    }
    $manifestPath = Join-Path $projectRoot 'release-assets\update.json'
    [IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    & $cargoPath run --locked --example sign-update -- sign $PrivateKey $manifestPath ($manifestPath + '.sig')
    if ($LASTEXITCODE -ne 0) { throw 'Signature échouée' }
    Write-Output 'Paquet et manifeste signés prêts dans release-assets. Aucune publication effectuée.'
} finally { Pop-Location }

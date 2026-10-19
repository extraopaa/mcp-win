$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoPath = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    $metadata = & $cargoPath metadata --format-version 1 --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Impossible de lire les dépendances Cargo' }
    $destination = Join-Path $projectRoot 'third-party-licenses'
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    $lines = [System.Collections.Generic.List[string]]::new()
    $lines.Add('# Dépendances et licences')
    $lines.Add('')
    $lines.Add('Inventaire Cargo.lock : dépendances de production, développement, compilation et plateformes. Les notices fournies par les crates sont recopiées dans third-party-licenses. Certaines crates proposent plusieurs licences au choix. Les mentions des polices sont également incluses.')
    $lines.Add('')
    $lines.Add('| Bibliothèque | Version | Licence déclarée | Source |')
    $lines.Add('| --- | --- | --- | --- |')
    foreach ($package in ($metadata.packages | Where-Object { $_.name -ne 'mcp-win' } | Sort-Object name,version)) {
        if (-not $package.license) { throw "Licence absente : $($package.name)" }
        $lines.Add("| $($package.name) | $($package.version) | $($package.license) | https://crates.io/crates/$($package.name)/$($package.version) |")
        $sourceRoot = Split-Path -Parent $package.manifest_path
        $packageDestination = Join-Path $destination "$($package.name)-$($package.version)"
        $files = Get-ChildItem -LiteralPath $sourceRoot -Recurse -File | Where-Object { $_.Name -match '(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT|OFL|UFL)' -or ($_.Directory.Name -eq 'fonts' -and $_.Extension -eq '.txt') }
        foreach ($file in $files) {
            $relativeName = $file.FullName.Substring($sourceRoot.Length).TrimStart('\','/')
            $target = Join-Path $packageDestination $relativeName
            New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
            Copy-Item -LiteralPath $file.FullName -Destination $target
        }
    }
    $lines | Set-Content -Encoding utf8 (Join-Path $projectRoot 'THIRD-PARTY-NOTICES.md')
    Write-Output "Notices générées pour $($metadata.packages.Count - 1) dépendances."
} finally { Pop-Location }

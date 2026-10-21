param([switch]$FetchOnly)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$cargo=Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
$rustup=Join-Path $env:USERPROFILE '.cargo/bin/rustup.exe'
$bindgen=Join-Path $env:USERPROFILE '.cargo/bin/wasm-bindgen.exe'
$sources=Get-Content -LiteralPath (Join-Path $root 'tools/office-web/sources.json') -Raw | ConvertFrom-Json
foreach($source in $sources.PSObject.Properties){
 $dest=Join-Path $root ('vendor/artcraft/'+$source.Name)
 if(-not(Test-Path -LiteralPath (Join-Path $dest '.git'))){
  New-Item -ItemType Directory -Force $dest | Out-Null
  & git -C $dest init --quiet
  & git -C $dest remote add origin ('https://github.com/storytold/'+$source.Name+'.git')
 }
 $head=& git -C $dest rev-parse --verify --quiet HEAD
 if($head -ne $source.Value){
  & git -C $dest fetch --depth 1 origin $source.Value
  if($LASTEXITCODE -ne 0){throw 'Sources bureautiques indisponibles'}
  & git -C $dest checkout --detach $source.Value
  if($LASTEXITCODE -ne 0){throw 'Revision bureautique incorrecte'}
 }
}
& node (Join-Path $PSScriptRoot 'office-brand.cjs')
if($LASTEXITCODE -ne 0){throw 'Adaptation graphique echouee'}
if($FetchOnly){return}
& $rustup target add wasm32-unknown-unknown
if(-not(Test-Path -LiteralPath $bindgen) -or (& $bindgen --version) -ne 'wasm-bindgen 0.2.129'){
 & $cargo install wasm-bindgen-cli --version 0.2.129 --locked
 if($LASTEXITCODE -ne 0){throw 'Installation du compilateur web echouee'}
}
& $cargo build --manifest-path (Join-Path $root 'tools/office-web/Cargo.toml') --target wasm32-unknown-unknown --release --locked
if($LASTEXITCODE -ne 0){throw 'Compilation bureautique echouee'}
$out=Join-Path $root 'ui/office/runtime'
New-Item -ItemType Directory -Force $out | Out-Null
& $bindgen (Join-Path $root 'tools/office-web/target/wasm32-unknown-unknown/release/mcp_win_office.wasm') --target web --out-dir $out --out-name mcp_win_office
if($LASTEXITCODE -ne 0){throw 'Preparation des editeurs echouee'}
Write-Output 'Les cinq editeurs bureautiques sont prets dans ui/office/runtime.'

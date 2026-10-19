$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$root = Join-Path (Split-Path -Parent $PSScriptRoot) 'vendor/engines'
$runtime = Join-Path $root 'runtime/tesseract'
if ((Test-Path -LiteralPath (Join-Path $runtime 'mcp-win-static-5.5.0.txt')) -and (Test-Path -LiteralPath (Join-Path $runtime 'tesseract.exe'))) { return }
$cmakeCommand = Get-Command cmake -ErrorAction SilentlyContinue
if ($cmakeCommand) { $cmake = $cmakeCommand.Source } else {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    $cmake = Join-Path $vs 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
}
if (-not (Test-Path -LiteralPath $cmake)) { throw 'CMake et les outils C++ MSVC sont requis pour compiler Tesseract' }
$packages = @(
    @('zlib','https://github.com/madler/zlib/archive/refs/tags/v1.3.1.zip','50B24B47BF19E1F35D2A21FF36D2A366638CDF958219A66F30CE0861201760E6'),
    @('png','https://github.com/pnggroup/libpng/archive/refs/tags/v1.6.44.zip','0B35B4BBDF454D589BF8195BC281FEFECC4B2529B42DDFBE8B6C108B986841F9'),
    @('leptonica','https://github.com/DanBloomberg/leptonica/archive/refs/tags/1.85.0.zip','59D37884ED57988309F8C000519D4024DF51949EA332F7A6FF53A6C29B4E1DB8'),
    @('tesseract','https://github.com/tesseract-ocr/tesseract/archive/refs/tags/5.5.0.zip','230007EE82A9836EBE5D5E4FD282665E753F3D6AE3B7C21949F8D86F38762A28')
)
New-Item -ItemType Directory -Force $root | Out-Null
foreach ($package in $packages) {
    $zip = Join-Path $root ($package[0] + '-source.zip')
    if (-not (Test-Path -LiteralPath $zip)) { Invoke-WebRequest $package[1] -OutFile $zip }
    if ((Get-FileHash -LiteralPath $zip).Hash -ne $package[2]) { throw "Source incorrecte : $($package[0])" }
    Expand-Archive -LiteralPath $zip -DestinationPath (Join-Path $root 'source') -Force
}
$prefix = (Join-Path $root 'native').Replace('\','/')
function Configure($source,$build,$options) {
    $ErrorActionPreference = 'Continue'
    & $cmake -S (Join-Path $root "source/$source") -B (Join-Path $root $build) -A x64 "-DCMAKE_INSTALL_PREFIX=$prefix" "-DCMAKE_PREFIX_PATH=$prefix" -DCMAKE_POLICY_DEFAULT_CMP0091=NEW -DCMAKE_POLICY_VERSION_MINIMUM=3.5 -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded @options
    if ($LASTEXITCODE -ne 0) { throw "Configuration échouée : $source" }
}
function Build($build,$target) {
    $ErrorActionPreference = 'Continue'
    & $cmake --build (Join-Path $root $build) --config Release --target $target --parallel 4
    if ($LASTEXITCODE -ne 0) { throw "Compilation échouée : $build" }
}
Configure 'zlib-1.3.1' 'build-zlib' @('-DZLIB_BUILD_EXAMPLES=OFF')
Build 'build-zlib' 'INSTALL'
Configure 'libpng-1.6.44' 'build-png' @("-DZLIB_LIBRARY=$prefix/lib/zlibstatic.lib",'-DPNG_SHARED=OFF','-DPNG_STATIC=ON','-DPNG_TESTS=OFF','-DPNG_TOOLS=OFF')
Build 'build-png' 'INSTALL'
Configure 'leptonica-1.85.0' 'build-lept' @("-DZLIB_LIBRARY=$prefix/lib/zlibstatic.lib","-DPNG_LIBRARY=$prefix/lib/libpng16_static.lib",'-DSW_BUILD=OFF','-DBUILD_SHARED_LIBS=OFF','-DBUILD_PROG=OFF','-DENABLE_GIF=OFF','-DENABLE_JPEG=OFF','-DENABLE_TIFF=OFF','-DENABLE_WEBP=OFF','-DENABLE_OPENJPEG=OFF','-DSTRICT_CONF=ON')
Build 'build-lept' 'INSTALL'
Configure 'tesseract-5.5.0' 'build-tess' @("-DZLIB_LIBRARY=$prefix/lib/zlibstatic.lib","-DPNG_LIBRARY=$prefix/lib/libpng16_static.lib",'-DSW_BUILD=OFF','-DBUILD_SHARED_LIBS=OFF','-DBUILD_TRAINING_TOOLS=OFF','-DBUILD_TESTS=OFF','-DGRAPHICS_DISABLED=ON','-DDISABLED_LEGACY_ENGINE=ON','-DOPENMP_BUILD=OFF','-DDISABLE_TIFF=ON','-DDISABLE_ARCHIVE=ON','-DDISABLE_CURL=ON','-DWIN32_MT_BUILD=ON')
Build 'build-tess' 'tesseract'
$exe = Get-ChildItem -LiteralPath (Join-Path $root 'build-tess') -Recurse -File -Filter 'tesseract.exe' | Where-Object FullName -Match 'Release' | Select-Object -First 1
if (-not $exe) { throw 'Exécutable Tesseract introuvable' }
New-Item -ItemType Directory -Force $runtime | Out-Null
Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $runtime 'tesseract.exe') -Force
$licenses = Join-Path $runtime 'licenses'
New-Item -ItemType Directory -Force $licenses | Out-Null
Copy-Item -LiteralPath (Join-Path $root 'source/tesseract-5.5.0/LICENSE') -Destination (Join-Path $licenses 'Tesseract-Apache-2.0.txt')
Copy-Item -LiteralPath (Join-Path $root 'source/tesseract-5.5.0/AUTHORS') -Destination (Join-Path $licenses 'Tesseract-AUTHORS.txt')
Copy-Item -LiteralPath (Join-Path $root 'source/leptonica-1.85.0/leptonica-license.txt') -Destination (Join-Path $licenses 'Leptonica.txt')
Copy-Item -LiteralPath (Join-Path $root 'source/libpng-1.6.44/LICENSE') -Destination (Join-Path $licenses 'libpng.txt')
Copy-Item -LiteralPath (Join-Path $root 'source/zlib-1.3.1/README') -Destination (Join-Path $licenses 'zlib-README.txt')
'Static MSVC Tesseract 5.5.0; Leptonica 1.85.0; libpng 1.6.44; zlib 1.3.1. Built from pinned open source; no curl/archive/training/legacy engine.' | Set-Content (Join-Path $runtime 'mcp-win-static-5.5.0.txt')

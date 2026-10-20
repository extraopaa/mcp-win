# Conversion des PNG fournis en ressources UI/Windows, sans redessiner le logo.
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$brandRoot = Split-Path -Parent $PSScriptRoot
$source = [Drawing.Bitmap]::new((Join-Path $brandRoot 'assets/brand/icon-original.png'))
try {
    $left=$source.Width; $top=$source.Height; $right=0; $bottom=0
    for ($y=0; $y -lt $source.Height; $y++) {
        for ($x=0; $x -lt $source.Width; $x++) {
            if ($source.GetPixel($x,$y).A -ge 16) {
                $left=[Math]::Min($left,$x); $top=[Math]::Min($top,$y)
                $right=[Math]::Max($right,$x); $bottom=[Math]::Max($bottom,$y)
            }
        }
    }
    if ($right -le $left -or $bottom -le $top) {throw 'Icône source vide'}
    $sourceRect=[Drawing.RectangleF]::new($left,$top,($right-$left+1),($bottom-$top+1))
    $frames=@()
    foreach ($size in @(16,20,24,32,40,48,64,128,256,512)) {
        $bitmap=[Drawing.Bitmap]::new($size,$size,[Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics=[Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.Clear([Drawing.Color]::Transparent)
            $graphics.CompositingQuality=[Drawing.Drawing2D.CompositingQuality]::HighQuality
            $graphics.InterpolationMode=[Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode=[Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $scale=($size*0.94)/[Math]::Max($sourceRect.Width,$sourceRect.Height)
            $width=$sourceRect.Width*$scale; $height=$sourceRect.Height*$scale
            $destination=[Drawing.RectangleF]::new(($size-$width)/2,($size-$height)/2,$width,$height)
            $graphics.DrawImage($source,$destination,$sourceRect,[Drawing.GraphicsUnit]::Pixel)
            $stream=[IO.MemoryStream]::new()
            try {
                $bitmap.Save($stream,[Drawing.Imaging.ImageFormat]::Png)
                $bytes=$stream.ToArray()
                if ($size -le 256) {$frames+=@{Size=$size;Bytes=$bytes}}
                if ($size -eq 32) {[IO.File]::WriteAllBytes((Join-Path $brandRoot 'ui/brand/favicon.png'),$bytes)}
                if ($size -eq 256) {[IO.File]::WriteAllBytes((Join-Path $brandRoot 'icons/icon.png'),$bytes)}
                if ($size -eq 512) {[IO.File]::WriteAllBytes((Join-Path $brandRoot 'ui/brand/icon.png'),$bytes)}
            } finally {$stream.Dispose()}
        } finally {$graphics.Dispose();$bitmap.Dispose()}
    }
    $file=[IO.File]::Create((Join-Path $brandRoot 'icons/icon.ico'))
    $writer=[IO.BinaryWriter]::new($file)
    try {
        $writer.Write([uint16]0);$writer.Write([uint16]1);$writer.Write([uint16]$frames.Count)
        $offset=6+16*$frames.Count
        foreach ($frame in $frames) {
            $dimension=if($frame.Size -eq 256){0}else{$frame.Size}
            $writer.Write([byte]$dimension);$writer.Write([byte]$dimension);$writer.Write([byte]0);$writer.Write([byte]0)
            $writer.Write([uint16]1);$writer.Write([uint16]32)
            $writer.Write([uint32]$frame.Bytes.Length);$writer.Write([uint32]$offset)
            $offset+=$frame.Bytes.Length
        }
        foreach ($frame in $frames) {$writer.Write([byte[]]$frame.Bytes)}
    } finally {$writer.Dispose()}
    Write-Output 'Logo fourni intégré : PNG UI 512 px, favicon 32 px, ICO Windows multi-résolution 16 à 256 px.'
} finally {$source.Dispose()}
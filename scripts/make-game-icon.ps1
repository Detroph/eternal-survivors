# Builds the game's icon from a source picture: the picture centred on a transparent square,
# scaled to fit (never stretched), saved as crates\esurv\assets\icon.png (256x256) and as
# crates\esurv\assets\icon.ico holding 16, 24, 32, 48, 64, 128 and 256 px PNG images.
# The game exe carries the .ico (crates\esurv\build.rs); the setup exe does not.
#
#   .\scripts\make-game-icon.ps1 -Source <picture.png>
param([Parameter(Mandatory = $true)][string]$Source)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$repo = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $repo 'crates\esurv\assets'
New-Item -ItemType Directory -Force $assets | Out-Null

$src = [System.Drawing.Bitmap]::FromFile((Resolve-Path $Source))

# One square image of `size` px: the picture scaled to fit, centred, transparent around it.
function Square([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $scale = [Math]::Min($size / $src.Width, $size / $src.Height)
    $w = [int][Math]::Round($src.Width * $scale)
    $h = [int][Math]::Round($src.Height * $scale)
    $g.DrawImage($src, [int](($size - $w) / 2), [int](($size - $h) / 2), $w, $h)
    $g.Dispose()
    $bmp
}

$big = Square 256
$big.Save((Join-Path $assets 'icon.png'), [System.Drawing.Imaging.ImageFormat]::Png)

# The .ico: a header, one 16-byte entry per image, then each image as PNG bytes.
$sizes = 16, 24, 32, 48, 64, 128, 256
$pngs = foreach ($s in $sizes) {
    $img = Square $s
    $ms = New-Object System.IO.MemoryStream
    $img.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $img.Dispose()
    , $ms.ToArray()
}
$out = New-Object System.IO.MemoryStream
$wr = New-Object System.IO.BinaryWriter($out)
$wr.Write([UInt16]0); $wr.Write([UInt16]1); $wr.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]
    $dim = if ($s -ge 256) { 0 } else { $s }
    $wr.Write([byte]$dim); $wr.Write([byte]$dim); $wr.Write([byte]0); $wr.Write([byte]0)
    $wr.Write([UInt16]1); $wr.Write([UInt16]32)
    $wr.Write([UInt32]$pngs[$i].Length); $wr.Write([UInt32]$offset)
    $offset += $pngs[$i].Length
}
foreach ($p in $pngs) { $wr.Write($p) }
$wr.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $assets 'icon.ico'), $out.ToArray())
$big.Dispose(); $src.Dispose()
Write-Host "make-game-icon: icon.png (256x256) and icon.ico ($($sizes -join ', ') px) in $assets"

# Genera ui/icon.png (64x64): fondo circular oscuro con tres círculos de color
# superpuestos. Uso, desde la raíz del repo: powershell -ExecutionPolicy Bypass -File ui/make-icon.ps1
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$size = 64
$bmp = New-Object System.Drawing.Bitmap $size, $size
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.Clear([System.Drawing.Color]::Transparent)

$background = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 30, 30, 36))
$g.FillEllipse($background, 0, 0, $size - 1, $size - 1)

$radius = 15
$circles = @(
    @{ X = 32; Y = 23; Color = [System.Drawing.Color]::FromArgb(210, 230, 57, 70) },
    @{ X = 23; Y = 39; Color = [System.Drawing.Color]::FromArgb(210, 42, 180, 120) },
    @{ X = 41; Y = 39; Color = [System.Drawing.Color]::FromArgb(210, 69, 123, 230) }
)
foreach ($c in $circles) {
    $brush = New-Object System.Drawing.SolidBrush $c.Color
    $g.FillEllipse($brush, $c.X - $radius, $c.Y - $radius, 2 * $radius, 2 * $radius)
    $brush.Dispose()
}

$out = Join-Path $PSScriptRoot 'icon.png'
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output "Escrito $out"

# Ícono .ico con el PNG embebido (formato admitido desde Windows Vista).
$pngBytes = [System.IO.File]::ReadAllBytes($out)
$ico = Join-Path $PSScriptRoot 'icon.ico'
$stream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter $stream
$writer.Write([uint16]0)                # reservado
$writer.Write([uint16]1)                # tipo: ícono
$writer.Write([uint16]1)                # cantidad de imágenes
$writer.Write([byte]$size)              # ancho
$writer.Write([byte]$size)              # alto
$writer.Write([byte]0)                  # paleta
$writer.Write([byte]0)                  # reservado
$writer.Write([uint16]1)                # planos
$writer.Write([uint16]32)               # bits por pixel
$writer.Write([uint32]$pngBytes.Length) # tamaño de la imagen
$writer.Write([uint32]22)               # posición de la imagen
$writer.Write($pngBytes)
$writer.Flush()
[System.IO.File]::WriteAllBytes($ico, $stream.ToArray())
$writer.Dispose()
Write-Output "Escrito $ico"

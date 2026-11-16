# Genera los iconos de la aplicación de escritorio.
# Uso: powershell -NoProfile -ExecutionPolicy Bypass -File scripts/generar-iconos.ps1
# Produce un .ico con entrada PNG de 256x256 y los PNG que espera Tauri.

Add-Type -AssemblyName System.Drawing

$destino = Join-Path $PSScriptRoot '..\desktop\src-tauri\icons'
New-Item -ItemType Directory -Force -Path $destino | Out-Null

function New-Lienzo([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    $margen = [Math]::Max(1, [int]($size * 0.06))
    $lado = $size - 2 * $margen
    $radio = [int]($lado * 0.22)
    $ruta = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $radio
    $ruta.AddArc($margen, $margen, $d, $d, 180, 90)
    $ruta.AddArc($margen + $lado - $d, $margen, $d, $d, 270, 90)
    $ruta.AddArc($margen + $lado - $d, $margen + $lado - $d, $d, $d, 0, 90)
    $ruta.AddArc($margen, $margen + $lado - $d, $d, $d, 90, 90)
    $ruta.CloseFigure()

    $fondo = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 18, 58, 94))
    $g.FillPath($fondo, $ruta)

    # Documento blanco con esquina doblada.
    $anchoDoc = $lado * 0.44
    $altoDoc = $lado * 0.56
    $x = ($size - $anchoDoc) / 2
    $y = ($size - $altoDoc) / 2
    $doblez = $anchoDoc * 0.28

    $doc = New-Object System.Drawing.Drawing2D.GraphicsPath
    $doc.AddLine($x, $y, ($x + $anchoDoc - $doblez), $y)
    $doc.AddLine(($x + $anchoDoc - $doblez), $y, ($x + $anchoDoc), ($y + $doblez))
    $doc.AddLine(($x + $anchoDoc), ($y + $doblez), ($x + $anchoDoc), ($y + $altoDoc))
    $doc.AddLine(($x + $anchoDoc), ($y + $altoDoc), $x, ($y + $altoDoc))
    $doc.CloseFigure()
    $g.FillPath([System.Drawing.Brushes]::White, $doc)

    $pliegue = New-Object System.Drawing.Drawing2D.GraphicsPath
    $pliegue.AddLine(($x + $anchoDoc - $doblez), $y, ($x + $anchoDoc - $doblez), ($y + $doblez))
    $pliegue.AddLine(($x + $anchoDoc - $doblez), ($y + $doblez), ($x + $anchoDoc), ($y + $doblez))
    $pliegue.CloseFigure()
    $g.FillPath((New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 199, 211, 224))), $pliegue)

    # Renglones del documento.
    $lapiz = New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(255, 120, 144, 168)), ([Math]::Max(1, $size / 128))
    for ($i = 0; $i -lt 3; $i++) {
        $ly = $y + $altoDoc * (0.42 + 0.16 * $i)
        $g.DrawLine($lapiz, ($x + $anchoDoc * 0.16), $ly, ($x + $anchoDoc * 0.84), $ly)
    }

    $lapiz.Dispose()
    $g.Dispose()
    return $bmp
}

function Save-Png([System.Drawing.Bitmap]$bmp, [string]$ruta) {
    $bmp.Save($ruta, [System.Drawing.Imaging.ImageFormat]::Png)
}

function New-Ico([string]$ruta, [System.Drawing.Bitmap]$bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $png = $ms.ToArray()
    $ms.Dispose()

    $fs = [System.IO.File]::Create($ruta)
    $bw = New-Object System.IO.BinaryWriter($fs)
    $bw.Write([uint16]0)          # reservado
    $bw.Write([uint16]1)          # tipo icono
    $bw.Write([uint16]1)          # número de imágenes
    $bw.Write([byte]0)            # ancho 256
    $bw.Write([byte]0)            # alto 256
    $bw.Write([byte]0)            # colores
    $bw.Write([byte]0)            # reservado
    $bw.Write([uint16]1)          # planos
    $bw.Write([uint16]32)         # bits por píxel
    $bw.Write([uint32]$png.Length)
    $bw.Write([uint32]22)         # desplazamiento tras cabecera + entrada
    $bw.Write($png)
    $bw.Flush()
    $bw.Dispose()
    $fs.Dispose()
}

$grande = New-Lienzo 256
New-Ico (Join-Path $destino 'icon.ico') $grande
Save-Png $grande (Join-Path $destino 'icon.png')
$grande.Dispose()

foreach ($tam in 32, 128, 256) {
    $bmp = New-Lienzo $tam
    $nombre = if ($tam -eq 128) { '128x128@2x.png' } else { "$tam" + 'x' + "$tam.png" }
    Save-Png $bmp (Join-Path $destino $nombre)
    $bmp.Dispose()
}

# Variante 128x128 real (el @2x debe ser 256).
$bmp128 = New-Lienzo 128
Save-Png $bmp128 (Join-Path $destino '128x128.png')
$bmp128.Dispose()
$bmp256 = New-Lienzo 256
Save-Png $bmp256 (Join-Path $destino '128x128@2x.png')
$bmp256.Dispose()
$bmp32 = New-Lienzo 32
Save-Png $bmp32 (Join-Path $destino '32x32.png')
$bmp32.Dispose()

Get-ChildItem $destino | Select-Object Name, Length

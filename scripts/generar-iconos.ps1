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
    $fondo.Dispose()

    # Carpeta blanca con insignia de verificación.
    $fx = $size * 0.23
    $fy = $size * 0.31
    $fw = $size * 0.48
    $fh = $size * 0.38
    $radioCarpeta = $size * 0.045
    $d = 2 * $radioCarpeta

    # Pestaña de la carpeta.
    $pestana = New-Object System.Drawing.Drawing2D.GraphicsPath
    $tw = $fw * 0.42
    $th = $size * 0.075
    $pestana.AddArc($fx, $fy - $th, $d, $d, 180, 90)
    $pestana.AddLine($fx + $tw - $radioCarpeta, $fy - $th, $fx + $tw, $fy - $th + $radioCarpeta)
    $pestana.AddLine($fx + $tw, $fy - $th + $radioCarpeta, $fx + $tw, $fy + $th)
    $pestana.AddLine($fx + $tw, $fy + $th, $fx, $fy + $th)
    $pestana.CloseFigure()
    $g.FillPath([System.Drawing.Brushes]::White, $pestana)

    # Cuerpo de la carpeta.
    $cuerpo = New-Object System.Drawing.Drawing2D.GraphicsPath
    $cuerpo.AddArc($fx, $fy, $d, $d, 180, 90)
    $cuerpo.AddArc($fx + $fw - $d, $fy, $d, $d, 270, 90)
    $cuerpo.AddArc($fx + $fw - $d, $fy + $fh - $d, $d, $d, 0, 90)
    $cuerpo.AddArc($fx, $fy + $fh - $d, $d, $d, 90, 90)
    $cuerpo.CloseFigure()
    $g.FillPath([System.Drawing.Brushes]::White, $cuerpo)

    # Frente de la carpeta en azul claro.
    $frente = New-Object System.Drawing.Drawing2D.GraphicsPath
    $fyFrente = $fy + $size * 0.055
    $fhFrente = $fh - $size * 0.055
    $frente.AddArc($fx, $fyFrente, $d, $d, 180, 90)
    $frente.AddArc($fx + $fw - $d, $fyFrente, $d, $d, 270, 90)
    $frente.AddArc($fx + $fw - $d, $fyFrente + $fhFrente - $d, $d, $d, 0, 90)
    $frente.AddArc($fx, $fyFrente + $fhFrente - $d, $d, $d, 90, 90)
    $frente.CloseFigure()
    $brochaFrente = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 226, 236, 246))
    $g.FillPath($brochaFrente, $frente)
    $brochaFrente.Dispose()

    # Insignia dorada con palomita.
    $bx = $fx + $fw * 0.74
    $by = $fy + $fh * 0.70
    $br = $size * 0.155
    $brochaInsignia = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 201, 162, 39))
    $g.FillEllipse($brochaInsignia, ($bx - $br), ($by - $br), (2 * $br), (2 * $br))
    $brochaInsignia.Dispose()

    $pluma = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ([Math]::Max(1.6, $size * 0.035))
    $pluma.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pluma.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawLine($pluma, ($bx - $br * 0.45), $by, ($bx - $br * 0.10), ($by + $br * 0.35))
    $g.DrawLine($pluma, ($bx - $br * 0.10), ($by + $br * 0.35), ($bx + $br * 0.50), ($by - $br * 0.38))
    $pluma.Dispose()
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

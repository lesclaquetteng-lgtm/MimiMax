# Generates the brand assets of CUBE Launcher: the application icon source and the two bitmaps the
# NSIS installer shows. Pure System.Drawing, no external dependency.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/make-brand.ps1
#   cd launcher/desktop && cargo tauri icon ../../tools/brand/icon-source.png
#
# The palette mirrors launcher/gui/src/ui/tokens.css: graphite surfaces and the voxel-green --brand.

Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$brand = Join-Path $root 'tools\brand'
$installer = Join-Path $root 'launcher\desktop\installer'
New-Item -ItemType Directory -Force $brand, $installer | Out-Null

$bg = [System.Drawing.ColorTranslator]::FromHtml('#0A0B0D')
$bgElevated = [System.Drawing.ColorTranslator]::FromHtml('#171A1F')
$brandLight = [System.Drawing.ColorTranslator]::FromHtml('#B4F27F')
$brandColor = [System.Drawing.ColorTranslator]::FromHtml('#93E05C')
$brandStrong = [System.Drawing.ColorTranslator]::FromHtml('#68D520')
$brandDeep = [System.Drawing.ColorTranslator]::FromHtml('#2F6410')
$brandInk = [System.Drawing.ColorTranslator]::FromHtml('#16230E')
$text = [System.Drawing.ColorTranslator]::FromHtml('#F2F4F6')
$muted = [System.Drawing.ColorTranslator]::FromHtml('#7D868F')
$dim = [System.Drawing.ColorTranslator]::FromHtml('#4E564F')

function New-RoundedPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
	$path = [System.Drawing.Drawing2D.GraphicsPath]::new()
	$d = $r * 2
	$path.AddArc($x, $y, $d, $d, 180, 90)
	$path.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
	$path.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
	$path.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
	$path.CloseFigure()
	return $path
}

function New-Canvas([int]$w, [int]$h) {
	$bitmap = [System.Drawing.Bitmap]::new($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
	$graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
	$graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
	return @{ Bitmap = $bitmap; Graphics = $graphics }
}

function New-Grid([System.Drawing.Graphics]$g, [int]$w, [int]$h, [int]$step, [int]$alpha) {
	$pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb($alpha, 255, 255, 255), 1)
	for ($i = 0; $i -le $w; $i += $step) { $g.DrawLine($pen, $i, 0, $i, $h) }
	for ($i = 0; $i -le $h; $i += $step) { $g.DrawLine($pen, 0, $i, $w, $i) }
	$pen.Dispose()
}

# An isometric voxel cube: three faces, one light source on top, sharp edges.
function New-VoxelCube([System.Drawing.Graphics]$g, [single]$cx, [single]$cyTop, [single]$w, [single]$hh, [single]$d) {
	$t = [System.Drawing.PointF]::new($cx, $cyTop - $hh)
	$r = [System.Drawing.PointF]::new($cx + $w, $cyTop)
	$b = [System.Drawing.PointF]::new($cx, $cyTop + $hh)
	$l = [System.Drawing.PointF]::new($cx - $w, $cyTop)
	$b2 = [System.Drawing.PointF]::new($cx, $cyTop + $hh + $d)
	$l2 = [System.Drawing.PointF]::new($cx - $w, $cyTop + $d)

	$topFace = @($t, $r, $b, $l)
	$leftFace = @($l, $b, $b2, $l2)
	$rightFace = @($b, $r, [System.Drawing.PointF]::new($cx + $w, $cyTop + $d), $b2)

	$topBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
		[System.Drawing.Rectangle]::new([int]($cx - $w), [int]($cyTop - $hh), [int]($w * 2), [int]($hh * 2)),
		$brandLight, $brandColor, [single]45)
	$g.FillPolygon($topBrush, $topFace)

	$leftBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
		[System.Drawing.Rectangle]::new([int]($cx - $w), [int]$cyTop, [int]($w * 2), [int]($d + $hh)),
		$brandStrong, $brandDeep, [single]90)
	$g.FillPolygon($leftBrush, $leftFace)

	$rightBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
		[System.Drawing.Rectangle]::new([int]$cx, [int]$cyTop, [int]($w * 2), [int]($d + $hh)),
		[System.Drawing.ColorTranslator]::FromHtml('#4FA31B'), [System.Drawing.ColorTranslator]::FromHtml('#1D4209'), [single]90)
	$g.FillPolygon($rightBrush, $rightFace)

	$edge = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(70, $brandInk), 1)
	$g.DrawLine($edge, $l, $b)
	$g.DrawLine($edge, $b, $r)
	$g.DrawLine($edge, $b, $b2)
	$g.DrawLine($edge, $l, $l2)
	$g.DrawLine($edge, $r, [System.Drawing.PointF]::new($cx + $w, $cyTop + $d))
	$g.DrawLine($edge, $l2, $b2)
	$edge.Dispose()
}

# Draws a string with extra tracking, the wordmark style of the interface.
function Draw-Tracked([System.Drawing.Graphics]$g, [string]$value, [System.Drawing.Font]$font, [System.Drawing.Brush]$brush, [single]$x, [single]$y, [single]$tracking) {
	$cursor = $x
	foreach ($ch in $value.ToCharArray()) {
		$g.DrawString([string]$ch, $font, $brush, $cursor, $y)
		$cursor += $g.MeasureString([string]$ch, $font).Width + $tracking
	}
}

# ------------------------------------------------------------------- the icon

$size = 1024
$canvas = New-Canvas $size $size
$g = $canvas.Graphics

$background = New-RoundedPath 0 0 $size $size 224
$bgBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
	[System.Drawing.Rectangle]::new(0, 0, $size, $size), $bgElevated, $bg, [single]90)
$g.FillPath($bgBrush, $background)
$g.SetClip($background)
New-Grid $g $size $size 128 7

# A soft halo behind the mark.
$halo = [System.Drawing.Drawing2D.GraphicsPath]::new()
$halo.AddEllipse(150, 190, 724, 724)
$haloBrush = [System.Drawing.Drawing2D.PathGradientBrush]::new($halo)
$haloBrush.CenterColor = [System.Drawing.Color]::FromArgb(58, $brandColor)
$haloBrush.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $brandColor))
$g.FillPath($haloBrush, $halo)

New-VoxelCube $g 512 470 300 173 200
$g.ResetClip()

$icon = Join-Path $brand 'icon-source.png'
$canvas.Bitmap.Save($icon, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$canvas.Bitmap.Dispose()

# ------------------------------------------------------------- header (150x57)

$canvas = New-Canvas 150 57
$g = $canvas.Graphics
$rect = [System.Drawing.Rectangle]::new(0, 0, 150, 57)
$g.FillRectangle([System.Drawing.Drawing2D.LinearGradientBrush]::new($rect, $bgElevated, $bg, [single]0), $rect)

New-VoxelCube $g 29 25 13 7.5 9

$wordBrush = [System.Drawing.SolidBrush]::new($text)
$g.DrawString('CUBE', [System.Drawing.Font]::new('Segoe UI', 15, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel),
	$wordBrush, 51, 9)
Draw-Tracked $g 'LAUNCHER' ([System.Drawing.Font]::new('Segoe UI', 7, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)) ([System.Drawing.SolidBrush]::new($muted)) 52 29 1.4

$header = Join-Path $installer 'header.bmp'
$canvas.Bitmap.Save($header, [System.Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose()
$canvas.Bitmap.Dispose()

# ------------------------------------------------ installer background (760x440)
#
# The NSIS template paints its whole window from this bitmap. The right side stays a flat colour
# because the template draws transparent labels on top of it; everything decorative lives in the
# left column.

$panelWidth = 760
$panelHeight = 440
$railWidth = 232

$canvas = New-Canvas $panelWidth $panelHeight
$g = $canvas.Graphics
$rect = [System.Drawing.Rectangle]::new(0, 0, $panelWidth, $panelHeight)
$g.FillRectangle([System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#0B0D10')), $rect)

$rail = [System.Drawing.Rectangle]::new(0, 0, $railWidth, $panelHeight)
$g.FillRectangle([System.Drawing.Drawing2D.LinearGradientBrush]::new($rail,
	[System.Drawing.ColorTranslator]::FromHtml('#131A15'), [System.Drawing.ColorTranslator]::FromHtml('#07090B'), [single]90), $rail)

# A faint grid: the world of the launcher.
$gridPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(10, 255, 255, 255), 1)
for ($i = 0; $i -lt $railWidth; $i += 16) { $g.DrawLine($gridPen, $i, 0, $i, $panelHeight) }
for ($i = 0; $i -lt $panelHeight; $i += 16) { $g.DrawLine($gridPen, 0, $i, $railWidth, $i) }
$gridPen.Dispose()

$halo = [System.Drawing.Drawing2D.GraphicsPath]::new()
$halo.AddEllipse(24, 76, 184, 184)
$haloBrush = [System.Drawing.Drawing2D.PathGradientBrush]::new($halo)
$haloBrush.CenterColor = [System.Drawing.Color]::FromArgb(64, $brandColor)
$haloBrush.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $brandColor))
$g.FillPath($haloBrush, $halo)

New-VoxelCube $g 116 152 62 36 42

$format = [System.Drawing.StringFormat]::new()
$format.Alignment = [System.Drawing.StringAlignment]::Center
$format.LineAlignment = [System.Drawing.StringAlignment]::Near

$g.DrawString('CUBE', [System.Drawing.Font]::new('Segoe UI', 26, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.SolidBrush]::new($text), [System.Drawing.RectangleF]::new(0, 268, $railWidth, 34), $format)
Draw-Tracked $g 'VOXEL EDITION' ([System.Drawing.Font]::new('Segoe UI', 8, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)) ([System.Drawing.SolidBrush]::new($brandColor)) 64 308 1.6

$linePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(40, 255, 255, 255), 1)
$g.DrawLine($linePen, 28, 342, 204, 342)
$linePen.Dispose()

$labelFont = [System.Drawing.Font]::new('Segoe UI', 7, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
Draw-Tracked $g 'INSTALLATION' $labelFont ([System.Drawing.SolidBrush]::new($dim)) 30 356 0.9

$smallFont = [System.Drawing.Font]::new('Segoe UI', 9, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$smallBrush = [System.Drawing.SolidBrush]::new($muted)
$g.DrawString('Minecraft 1.21.11', $smallFont, $smallBrush, 30, 374)
$g.DrawString('Fabric 0.20.1', $smallFont, $smallBrush, 30, 390)
$g.DrawString('version 1.0.0', [System.Drawing.Font]::new('Segoe UI', 8, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.SolidBrush]::new($dim), 30, 408)

# A one-pixel seam between the brand rail and the content area.
$seam = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(70, $brandColor), 1)
$g.DrawLine($seam, $railWidth - 1, 0, $railWidth - 1, $panelHeight)
$seam.Dispose()

$accent = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(90, $brandColor), 1)
$g.DrawLine($accent, $railWidth, 48, $panelWidth, 48)
$accent.Dispose()

$sidebar = Join-Path $installer 'sidebar.bmp'
$canvas.Bitmap.Save($sidebar, [System.Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose()
$canvas.Bitmap.Dispose()

Write-Output "icon=$icon"
Write-Output "header=$header $((Get-Item $header).Length) bytes"
Write-Output "panel=$sidebar $((Get-Item $sidebar).Length) bytes"

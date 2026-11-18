# Generates the brand assets of the launcher: the application icon source and the two bitmaps the
# NSIS installer shows. Pure System.Drawing, no external dependency.

Add-Type -AssemblyName System.Drawing

$root = 'C:\Users\ee\Desktop\MimiMax'
$brand = Join-Path $root 'tools\brand'
$installer = Join-Path $root 'launcher\desktop\installer'
New-Item -ItemType Directory -Force $brand, $installer | Out-Null

$accent = [System.Drawing.ColorTranslator]::FromHtml('#6C5CE7')
$accentLight = [System.Drawing.ColorTranslator]::FromHtml('#9B8CFF')
$cyan = [System.Drawing.ColorTranslator]::FromHtml('#3DD6C6')
$dark = [System.Drawing.ColorTranslator]::FromHtml('#0B0C12')
$darkElevated = [System.Drawing.ColorTranslator]::FromHtml('#171A26')
$text = [System.Drawing.ColorTranslator]::FromHtml('#F5F6FA')
$muted = [System.Drawing.ColorTranslator]::FromHtml('#8C90A6')

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

function New-Diamond([single]$cx, [single]$cy, [single]$radius) {
	$points = @(
		[System.Drawing.PointF]::new($cx, $cy - $radius),
		[System.Drawing.PointF]::new($cx + $radius, $cy),
		[System.Drawing.PointF]::new($cx, $cy + $radius),
		[System.Drawing.PointF]::new($cx - $radius, $cy)
	)
	return $points
}

# ---------------------------------------------------------------- the icon

$size = 1024
$canvas = New-Canvas $size $size
$g = $canvas.Graphics

$background = New-RoundedPath 0 0 $size $size 224
$brush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
	[System.Drawing.Rectangle]::new(0, 0, $size, $size),
	[System.Drawing.ColorTranslator]::FromHtml('#1B1E2C'),
	$dark,
	[single]90
)
$g.FillPath($brush, $background)

# A soft halo behind the mark.
$halo = [System.Drawing.Drawing2D.GraphicsPath]::new()
$halo.AddEllipse(150, 150, 724, 724)
$haloBrush = [System.Drawing.Drawing2D.PathGradientBrush]::new($halo)
$haloBrush.CenterColor = [System.Drawing.Color]::FromArgb(70, $accent)
$haloBrush.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $accent))
$g.FillPath($haloBrush, $halo)

$diamond = New-Diamond 512 512 330
$glow = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
	[System.Drawing.Rectangle]::new(180, 180, 664, 664),
	$accentLight,
	$cyan,
	[single]45
)
$g.FillPolygon($glow, $diamond)

# A darker inner diamond gives the mark a readable edge at 16 px.
$inner = New-Diamond 512 512 246
$innerBrush = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
	[System.Drawing.Rectangle]::new(260, 260, 504, 504),
	[System.Drawing.ColorTranslator]::FromHtml('#241E52'),
	[System.Drawing.ColorTranslator]::FromHtml('#0E0D22'),
	[single]90
)
$g.FillPolygon($innerBrush, $inner)

$font = [System.Drawing.Font]::new('Segoe UI', 330, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$format = [System.Drawing.StringFormat]::new()
$format.Alignment = [System.Drawing.StringAlignment]::Center
$format.LineAlignment = [System.Drawing.StringAlignment]::Center
$g.DrawString('A', $font, [System.Drawing.Brushes]::White, [System.Drawing.RectangleF]::new(0, -30, $size, $size), $format)

# The accent bar under the letter, like a cursor.
$barBrush = [System.Drawing.SolidBrush]::new($cyan)
$g.FillRectangle($barBrush, 452, 642, 120, 16)

$icon = Join-Path $brand 'icon-source.png'
$canvas.Bitmap.Save($icon, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$canvas.Bitmap.Dispose()

# ------------------------------------------------------------- header (150x57)

$canvas = New-Canvas 150 57
$g = $canvas.Graphics
$rect = [System.Drawing.Rectangle]::new(0, 0, 150, 57)
$g.FillRectangle([System.Drawing.Drawing2D.LinearGradientBrush]::new($rect, $darkElevated, $dark, [single]0), $rect)

$mark = New-Diamond 28 28 15
$g.FillPolygon([System.Drawing.Drawing2D.LinearGradientBrush]::new(
	[System.Drawing.Rectangle]::new(10, 10, 36, 36), $accentLight, $cyan, [single]45), $mark)

$g.DrawString('A', [System.Drawing.Font]::new('Segoe UI', 17, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.Brushes]::White, 22, 19)

$g.DrawString('AETHER', [System.Drawing.Font]::new('Segoe UI', 15, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.SolidBrush]::new($text), 48, 10)
$g.DrawString('LAUNCHER', [System.Drawing.Font]::new('Segoe UI', 8, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.SolidBrush]::new($muted), 49, 28)

$header = Join-Path $installer 'header.bmp'
$canvas.Bitmap.Save($header, [System.Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose()
$canvas.Bitmap.Dispose()

# ------------------------------------------------------------ sidebar (164x314)

$canvas = New-Canvas 164 314
$g = $canvas.Graphics
$rect = [System.Drawing.Rectangle]::new(0, 0, 164, 314)
$g.FillRectangle([System.Drawing.Drawing2D.LinearGradientBrush]::new($rect,
	[System.Drawing.ColorTranslator]::FromHtml('#1A1D2D'), $dark, [single]90), $rect)

# A faint grid, the "world" of the client.
$gridPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(14, 255, 255, 255), 1)
for ($i = 0; $i -lt 164; $i += 18) { $g.DrawLine($gridPen, $i, 0, $i, 314) }
for ($i = 0; $i -lt 314; $i += 18) { $g.DrawLine($gridPen, 0, $i, 164, $i) }

$halo = [System.Drawing.Drawing2D.GraphicsPath]::new()
$halo.AddEllipse(22, 44, 120, 120)
$haloBrush = [System.Drawing.Drawing2D.PathGradientBrush]::new($halo)
$haloBrush.CenterColor = [System.Drawing.Color]::FromArgb(80, $accent)
$haloBrush.SurroundColors = @([System.Drawing.Color]::FromArgb(0, $accent))
$g.FillPath($haloBrush, $halo)

$mark = New-Diamond 82 104 52
$g.FillPolygon([System.Drawing.Drawing2D.LinearGradientBrush]::new(
	[System.Drawing.Rectangle]::new(30, 52, 104, 104), $accentLight, $cyan, [single]45), $mark)
$g.DrawString('A', [System.Drawing.Font]::new('Segoe UI', 62, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.Brushes]::White, 68, 67)

$titleFont = [System.Drawing.Font]::new('Segoe UI', 21, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString('AETHER', $titleFont, [System.Drawing.SolidBrush]::new($text), 44, 182)

$subFont = [System.Drawing.Font]::new('Segoe UI', 9, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString('C L I E N T', $subFont, [System.Drawing.SolidBrush]::new($accentLight), 47, 212)

$linePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(40, 255, 255, 255), 1)
$g.DrawLine($linePen, 20, 236, 144, 236)

$smallFont = [System.Drawing.Font]::new('Segoe UI', 9, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
$g.DrawString('Installation', $smallFont, [System.Drawing.SolidBrush]::new($muted), 22, 248)
$g.DrawString('Minecraft 1.21.11', $smallFont, [System.Drawing.SolidBrush]::new($muted), 22, 264)
$g.DrawString('Fabric 0.20.1', $smallFont, [System.Drawing.SolidBrush]::new($muted), 22, 280)
$g.DrawString('version 1.0.0', [System.Drawing.Font]::new('Segoe UI', 8, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel),
	[System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml('#5E6175')), 22, 296)

$sidebar = Join-Path $installer 'sidebar.bmp'
$canvas.Bitmap.Save($sidebar, [System.Drawing.Imaging.ImageFormat]::Bmp)
$g.Dispose()
$canvas.Bitmap.Dispose()

Write-Output "icon=$icon"
Write-Output "header=$header $((Get-Item $header).Length) bytes"
Write-Output "sidebar=$sidebar $((Get-Item $sidebar).Length) bytes"

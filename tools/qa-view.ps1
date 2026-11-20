param([string]$In, [string]$Out, [int]$Width = 1100)
Add-Type -AssemblyName System.Drawing
$source = [System.Drawing.Image]::FromFile($In)
$height = [int]($source.Height * $Width / $source.Width)
$bitmap = New-Object System.Drawing.Bitmap($Width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$graphics.DrawImage($source, 0, 0, $Width, $height)
$graphics.Dispose()
$bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()
$source.Dispose()
Write-Output ("view " + $Out + " " + $Width + "x" + $height)

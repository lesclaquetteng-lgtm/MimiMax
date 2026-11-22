param(
	[string]$Title = 'Minecraft',
	[string]$Output = 'window.png'
)

# Captures a window through PrintWindow, so a window covered by a game is still readable.
# CopyFromScreen would capture whatever is in front of it instead.

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class PrintWindowCapture {
	[StructLayout(LayoutKind.Sequential)]
	public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
	[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
}
"@

$process = Get-Process |
	Where-Object { $_.MainWindowTitle -like "*$Title*" -and $_.MainWindowHandle -ne 0 } |
	Select-Object -First 1

if (-not $process) {
	Write-Error "no window matches $Title"
	exit 1
}

$handle = $process.MainWindowHandle
[PrintWindowCapture]::ShowWindow($handle, 9) | Out-Null
[PrintWindowCapture]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 800

$rect = New-Object PrintWindowCapture+RECT
[PrintWindowCapture]::GetWindowRect($handle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

if ($width -le 0 -or $height -le 0) {
	Write-Error "the window has no surface ($width x $height)"
	exit 1
}

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()

# 0x00000002 is PW_RENDERFULLCONTENT, required by the windows that compose with DirectComposition.
$printed = [PrintWindowCapture]::PrintWindow($handle, $hdc, 0x00000002)

$graphics.ReleaseHdc($hdc)
$graphics.Dispose()

if (-not $printed) {
	Write-Error "the window refused to be printed"
	$bitmap.Dispose()
	exit 1
}

$bitmap.Save($Output, [System.Drawing.Imaging.ImageFormat]::Png)
$bitmap.Dispose()

Write-Output "captured $($process.MainWindowTitle) ${width}x${height} -> $Output"

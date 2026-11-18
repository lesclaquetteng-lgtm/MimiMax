param(
	[string]$Title = 'Minecraft',
	[string]$Output = 'window.png'
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Win32Window {
	[StructLayout(LayoutKind.Sequential)]
	public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
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
[Win32Window]::ShowWindow($handle, 9) | Out-Null
[Win32Window]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 700

$rect = New-Object Win32Window+RECT
[Win32Window]::GetWindowRect($handle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

if ($width -le 0 -or $height -le 0) {
	Write-Error "the window has no surface ($width x $height)"
	exit 1
}

$bitmap = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
$bitmap.Save($Output, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bitmap.Dispose()

Write-Output "captured $($process.MainWindowTitle) ${width}x${height} -> $Output"

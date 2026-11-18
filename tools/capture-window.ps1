param(
    [string]$Exe = "C:\Users\ee\Desktop\MimiMax\launcher\desktop\target\release\aether-desktop.exe",
    [string]$Out = "C:\Users\ee\Desktop\MimiMax\tools\theme-shot.png",
    [int]$WaitMs = 9000,
    [int]$MaxWidth = 1024
)

# Captures the real window of the launcher, once it has painted its first frame.
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class Win32Cap {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
}
"@

$p = Start-Process -FilePath $Exe -PassThru
try {
    Start-Sleep -Milliseconds $WaitMs
    $p.Refresh()
    $handle = $p.MainWindowHandle
    if ($handle -eq [IntPtr]::Zero) {
        for ($i = 0; $i -lt 20 -and $handle -eq [IntPtr]::Zero; $i++) {
            Start-Sleep -Milliseconds 500
            $p.Refresh()
            $handle = $p.MainWindowHandle
        }
    }
    if ($handle -eq [IntPtr]::Zero) { throw "the window never appeared" }

    [Win32Cap]::ShowWindow($handle, 9) | Out-Null
    [Win32Cap]::BringWindowToTop($handle) | Out-Null
    [Win32Cap]::SetForegroundWindow($handle) | Out-Null
    Start-Sleep -Milliseconds 1500

    $rect = New-Object Win32Cap+RECT
    [Win32Cap]::GetWindowRect($handle, [ref]$rect) | Out-Null
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { throw "the window has no size" }

    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($width, $height)))
    $graphics.Dispose()

    if ($width -gt $MaxWidth) {
        $scale = $MaxWidth / $width
        $small = New-Object System.Drawing.Bitmap $MaxWidth, ([int]($height * $scale))
        $resized = [System.Drawing.Graphics]::FromImage($small)
        $resized.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $resized.DrawImage($bitmap, 0, 0, $small.Width, $small.Height)
        $resized.Dispose()
        $bitmap.Dispose()
        $bitmap = $small
    }

    $bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host ("saved {0} ({1}x{2})" -f $Out, $bitmap.Width, $bitmap.Height)
    $bitmap.Dispose()
}
finally {
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
}

param(
    [string]$Title = 'Minecraft',
    [string]$From = "0,0",
    [string]$To = "0,0",
    [int]$Steps = 12
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct DragRect { public int Left; public int Top; public int Right; public int Bottom; }
public class DragNative {
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out DragRect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
}
"@ -ReferencedAssemblies System.Drawing

$proc = Get-Process |
    Where-Object { $_.MainWindowTitle -like "*$Title*" -and $_.MainWindowHandle -ne 0 } |
    Select-Object -First 1
if (-not $proc) { Write-Error "no window matches $Title"; exit 1 }

$hwnd = $proc.MainWindowHandle
$origin = New-Object System.Drawing.Point(0, 0)
[DragNative]::ClientToScreen($hwnd, [ref]$origin) | Out-Null
[DragNative]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 400

$a = $From.Split(","); $b = $To.Split(",")
$x0 = [int]$a[0]; $y0 = [int]$a[1]; $x1 = [int]$b[0]; $y1 = [int]$b[1]

[DragNative]::SetCursorPos($origin.X + $x0, $origin.Y + $y0) | Out-Null
Start-Sleep -Milliseconds 200
[DragNative]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 120
for ($i = 1; $i -le $Steps; $i++) {
    $x = $x0 + ($x1 - $x0) * $i / $Steps
    $y = $y0 + ($y1 - $y0) * $i / $Steps
    [DragNative]::SetCursorPos($origin.X + [int]$x, $origin.Y + [int]$y) | Out-Null
    Start-Sleep -Milliseconds 40
}
Start-Sleep -Milliseconds 120
[DragNative]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Write-Output "dragged $From -> $To"
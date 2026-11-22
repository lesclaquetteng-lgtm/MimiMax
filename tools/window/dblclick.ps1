param(
    [string]$TitleLike = "*Minecraft*",
    [string]$Click = "300,118"
)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct DRect { public int Left; public int Top; public int Right; public int Bottom; }
public class DNative {
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out DRect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
}
"@ -ReferencedAssemblies System.Drawing
$p = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitleLike } |
    Select-Object -First 1
if (-not $p) { Write-Output "no window"; exit 1 }
$hwnd = $p.MainWindowHandle
$rect = New-Object DRect
[DNative]::GetClientRect($hwnd, [ref]$rect) | Out-Null
$origin = New-Object System.Drawing.Point(0, 0)
[DNative]::ClientToScreen($hwnd, [ref]$origin) | Out-Null
$parts = $Click.Split(",")
$sx = $origin.X + [int]$parts[0]
$sy = $origin.Y + [int]$parts[1]
[DNative]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 250
[DNative]::SetCursorPos($sx, $sy) | Out-Null
Start-Sleep -Milliseconds 150
for ($i = 0; $i -lt 2; $i++) {
    [DNative]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [DNative]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 70
}
Write-Output ("double click at " + $sx + "," + $sy + " (client " + $Click + ")")

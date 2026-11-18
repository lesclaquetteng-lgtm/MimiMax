param(
    [string]$TitleLike = "*Aether Launcher*",
    [string]$Out = "",
    [string]$Click = "",
    [int]$Button = 0,
    [int]$Wheel = 0
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct GuiRect { public int Left; public int Top; public int Right; public int Bottom; }
public class GuiNative {
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out GuiRect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
}
"@ -ReferencedAssemblies System.Drawing

$proc = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitleLike } |
    Select-Object -First 1

if (-not $proc) {
    Write-Output "no window matching $TitleLike"
    exit 1
}

$hwnd = $proc.MainWindowHandle
$rect = New-Object GuiRect
[GuiNative]::GetClientRect($hwnd, [ref]$rect) | Out-Null
$origin = New-Object System.Drawing.Point(0, 0)
[GuiNative]::ClientToScreen($hwnd, [ref]$origin) | Out-Null

$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
Write-Output "window '$($proc.MainWindowTitle)' pid $($proc.Id) client ${width}x${height} at $($origin.X),$($origin.Y)"

if ($Out -ne "") {
    [GuiNative]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 400
    $bitmap = New-Object System.Drawing.Bitmap($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($origin.X, $origin.Y, 0, 0, $bitmap.Size)
    $bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    Write-Output "saved $Out"
}

if ($Click -ne "") {
    $parts = $Click.Split(",")
    $x = [int]$parts[0]
    $y = [int]$parts[1]

    [GuiNative]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 250
    [GuiNative]::SetCursorPos($origin.X + $x, $origin.Y + $y) | Out-Null
    Start-Sleep -Milliseconds 150
    if ($Button -eq 1) {
        [GuiNative]::mouse_event(0x0008, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
        [GuiNative]::mouse_event(0x0010, 0, 0, 0, [IntPtr]::Zero)
        Write-Output "right click at $x,$y"
    } else {
        [GuiNative]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
        [GuiNative]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
        Write-Output "left click at $x,$y"
    }
}

if ($Wheel -ne 0) {
    [GuiNative]::SetForegroundWindow($hwnd) | Out-Null
    Start-Sleep -Milliseconds 250
    if ($Wheel -lt 0) { $delta = [uint32](4294967296 + $Wheel) } else { $delta = [uint32]$Wheel }
    [GuiNative]::mouse_event(0x0800, 0, 0, $delta, [IntPtr]::Zero)
    Write-Output "wheel $Wheel"
}

Start-Sleep -Milliseconds 300

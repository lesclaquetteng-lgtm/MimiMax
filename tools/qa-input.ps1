param(
    [string]$TitleLike = "*Minecraft*",
    [string]$Click = "",
    [string]$Drag = "",
    [int]$Vk = 0,
    [int]$Scan = 0,
    [string]$Shot = "",
    [int]$Steps = 20,
    [int]$StepMs = 45,
    [switch]$Probe,
    [switch]$Focus,
    [switch]$Restore,
    [int]$PostKey = 0,
    [string]$PostClick = "",
    [string]$PostDrag = ""
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct QRect { public int Left; public int Top; public int Right; public int Bottom; }
public class QNative {
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out QRect r);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out QRect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint message, IntPtr wParam, IntPtr lParam);
}
"@ -ReferencedAssemblies System.Drawing

$proc = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitleLike } |
    Select-Object -First 1
if (-not $proc) { Write-Output "no window"; exit 1 }
$hwnd = $proc.MainWindowHandle

$client = New-Object QRect
[QNative]::GetClientRect($hwnd, [ref]$client) | Out-Null

function Get-ClientOrigin {
    $point = New-Object System.Drawing.Point(0, 0)
    [QNative]::ClientToScreen($hwnd, [ref]$point) | Out-Null
    return @($point.X, $point.Y)
}

if ($Restore) {
    # ShowWindow without activating: the window comes back on screen, the player's foreground
    # application keeps the keyboard and the pointer.
    [QNative]::ShowWindow($hwnd, 4) | Out-Null
    Start-Sleep -Milliseconds 600
    $restored = New-Object QRect
    [QNative]::GetClientRect($hwnd, [ref]$restored) | Out-Null
    Write-Output ("restored " + $restored.Right + "x" + $restored.Bottom + " minimized=" + [QNative]::IsIconic($hwnd))
    exit 0
}

if ($Probe) {
    $origin = Get-ClientOrigin
    $foreground = [QNative]::GetForegroundWindow()
    $foregroundTitle = (Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -eq $foreground } | Select-Object -First 1).MainWindowTitle
    Write-Output ("client " + $client.Right + "x" + $client.Bottom + " at " + $origin[0] + "," + $origin[1] +
        " | foreground: " + $foregroundTitle)
    exit 0
}

# A click only reaches the game when the window owns the foreground; attaching to its input queue
# is what makes SetForegroundWindow succeed from a background process.
$targetThread = [QNative]::GetWindowThreadProcessId($hwnd, [IntPtr]::Zero)
$ownThread = [QNative]::GetCurrentThreadId()
try {
    $shell = New-Object -ComObject WScript.Shell
    $shell.AppActivate($proc.Id) | Out-Null
} catch { }
[QNative]::AttachThreadInput($ownThread, $targetThread, $true) | Out-Null
[QNative]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 350
[QNative]::AttachThreadInput($ownThread, $targetThread, $false) | Out-Null

function Move-To([int]$x, [int]$y) {
    $origin = Get-ClientOrigin
    [QNative]::SetCursorPos(($origin[0] + $x), ($origin[1] + $y)) | Out-Null
}

if ($Click -ne "") {
    $parts = $Click.Split(",")
    Move-To ([int]$parts[0]) ([int]$parts[1])
    Start-Sleep -Milliseconds 140
    [QNative]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 70
    [QNative]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    Write-Output ("click " + $Click)
}

if ($Drag -ne "") {
    $parts = $Drag.Split(",")
    if ($parts.Count -ne 4) { Write-Output "drag needs x1,y1,x2,y2"; exit 1 }
    $x1 = [int]$parts[0]; $y1 = [int]$parts[1]; $x2 = [int]$parts[2]; $y2 = [int]$parts[3]
    Move-To $x1 $y1
    Start-Sleep -Milliseconds 200
    [QNative]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 200
    for ($i = 1; $i -le $Steps; $i++) {
        Move-To ([int]($x1 + ($x2 - $x1) * $i / $Steps)) ([int]($y1 + ($y2 - $y1) * $i / $Steps))
        Start-Sleep -Milliseconds $StepMs
    }
    Start-Sleep -Milliseconds 200
    [QNative]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
    Write-Output ("drag " + $Drag + " in " + $Steps + " steps")
}

if ($Vk -ne 0) {
    [QNative]::keybd_event([byte]$Vk, [byte]$Scan, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 90
    [QNative]::keybd_event([byte]$Vk, [byte]$Scan, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    Write-Output ("key " + $Vk)
}

if ($PostKey -ne 0) {
    # Posted straight to the window: no foreground change, so the input cannot leak into another
    # application. The high word of lParam carries the scan code GLFW reads back.
    $scanCode = $Scan
    $down = [IntPtr](1 -bor ($scanCode -shl 16))
    $up = [IntPtr](0xC0000001 -bor ($scanCode -shl 16))
    [QNative]::PostMessage($hwnd, 0x0100, [IntPtr]$PostKey, $down) | Out-Null
    Start-Sleep -Milliseconds 90
    [QNative]::PostMessage($hwnd, 0x0101, [IntPtr]$PostKey, $up) | Out-Null
    Start-Sleep -Milliseconds 150
    Write-Output ("post key " + $PostKey)
}

if ($PostClick -ne "") {
    $parts = $PostClick.Split(",")
    $x = [int]$parts[0]; $y = [int]$parts[1]
    $position = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
    [QNative]::PostMessage($hwnd, 0x0200, [IntPtr]::Zero, $position) | Out-Null
    Start-Sleep -Milliseconds 120
    [QNative]::PostMessage($hwnd, 0x0201, [IntPtr]1, $position) | Out-Null
    Start-Sleep -Milliseconds 80
    [QNative]::PostMessage($hwnd, 0x0202, [IntPtr]::Zero, $position) | Out-Null
    Start-Sleep -Milliseconds 120
    Write-Output ("post click " + $PostClick)
}

if ($PostDrag -ne "") {
    $parts = $PostDrag.Split(",")
    $x1 = [int]$parts[0]; $y1 = [int]$parts[1]; $x2 = [int]$parts[2]; $y2 = [int]$parts[3]
    $start = [IntPtr](($y1 -shl 16) -bor ($x1 -band 0xFFFF))
    [QNative]::PostMessage($hwnd, 0x0200, [IntPtr]::Zero, $start) | Out-Null
    Start-Sleep -Milliseconds 150
    [QNative]::PostMessage($hwnd, 0x0201, [IntPtr]1, $start) | Out-Null
    Start-Sleep -Milliseconds 150
    for ($i = 1; $i -le $Steps; $i++) {
        $px = [int]($x1 + ($x2 - $x1) * $i / $Steps)
        $py = [int]($y1 + ($y2 - $y1) * $i / $Steps)
        $move = [IntPtr](($py -shl 16) -bor ($px -band 0xFFFF))
        [QNative]::PostMessage($hwnd, 0x0200, [IntPtr]1, $move) | Out-Null
        Start-Sleep -Milliseconds $StepMs
    }
    $end = [IntPtr](($y2 -shl 16) -bor ($x2 -band 0xFFFF))
    Start-Sleep -Milliseconds 150
    [QNative]::PostMessage($hwnd, 0x0202, [IntPtr]::Zero, $end) | Out-Null
    Start-Sleep -Milliseconds 150
    Write-Output ("post drag " + $PostDrag + " in " + $Steps + " steps")
}

if ($Shot -ne "") {
    $window = New-Object QRect
    [QNative]::GetWindowRect($hwnd, [ref]$window) | Out-Null
    $fullWidth = $window.Right - $window.Left
    $fullHeight = $window.Bottom - $window.Top
    $full = New-Object System.Drawing.Bitmap($fullWidth, $fullHeight)
    $fullGraphics = [System.Drawing.Graphics]::FromImage($full)
    $hdc = $fullGraphics.GetHdc()
    [QNative]::PrintWindow($hwnd, $hdc, 2) | Out-Null
    $fullGraphics.ReleaseHdc($hdc)
    $fullGraphics.Dispose()

    # The window bitmap includes the frame; the client area is what the coordinates refer to.
    $origin = Get-ClientOrigin
    $offsetX = $origin[0] - $window.Left
    $offsetY = $origin[1] - $window.Top
    $clientBitmap = New-Object System.Drawing.Bitmap($client.Right, $client.Bottom)
    $clientGraphics = [System.Drawing.Graphics]::FromImage($clientBitmap)
    $destination = New-Object System.Drawing.Rectangle(0, 0, $client.Right, $client.Bottom)
    $source = New-Object System.Drawing.Rectangle($offsetX, $offsetY, $client.Right, $client.Bottom)
    $clientGraphics.DrawImage($full, $destination, $source, [System.Drawing.GraphicsUnit]::Pixel)
    $clientGraphics.Dispose()
    $clientBitmap.Save($Shot, [System.Drawing.Imaging.ImageFormat]::Png)
    $clientBitmap.Dispose()
    $full.Dispose()
    Write-Output ("shot " + $client.Right + "x" + $client.Bottom + " -> " + $Shot)
}

[QNative]::AttachThreadInput($ownThread, $targetThread, $false) | Out-Null

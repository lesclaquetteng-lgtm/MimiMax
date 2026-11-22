param(
    [string]$TitleLike = "*Minecraft*",
    [string]$Click = "",
    [int]$Button = 0,
    [int]$Key = 0,
    [int]$Scan = 0,
    [int]$Wheel = 0,
    [string]$Out = ""
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public struct GuiRect { public int Left; public int Top; public int Right; public int Bottom; }
public class PostNative {
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out GuiRect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref System.Drawing.Point p);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@ -ReferencedAssemblies System.Drawing

$proc = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitleLike } |
    Select-Object -First 1
if (-not $proc) { Write-Output "no window matching $TitleLike"; exit 1 }
$hwnd = $proc.MainWindowHandle
$rect = New-Object GuiRect
[PostNative]::GetClientRect($hwnd, [ref]$rect) | Out-Null
$origin = New-Object System.Drawing.Point(0, 0)
[PostNative]::ClientToScreen($hwnd, [ref]$origin) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
Write-Output "window '$($proc.MainWindowTitle)' pid $($proc.Id) client ${width}x${height} at $($origin.X),$($origin.Y)"

[PostNative]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 200

if ($Click -ne "") {
    $parts = $Click.Split(",")
    $x = [int]$parts[0]; $y = [int]$parts[1]
    $lp = [IntPtr](($y -shl 16) -bor ($x -band 0xFFFF))
    if ($Button -eq 1) {
        [PostNative]::PostMessage($hwnd, 0x0204, [IntPtr]0x0002, $lp) | Out-Null  # WM_RBUTTONDOWN
        Start-Sleep -Milliseconds 60
        [PostNative]::PostMessage($hwnd, 0x0205, [IntPtr]0, $lp) | Out-Null        # WM_RBUTTONUP
        Write-Output "posted right click at $x,$y"
    } else {
        [PostNative]::PostMessage($hwnd, 0x0201, [IntPtr]0x0001, $lp) | Out-Null  # WM_LBUTTONDOWN
        Start-Sleep -Milliseconds 60
        [PostNative]::PostMessage($hwnd, 0x0202, [IntPtr]0, $lp) | Out-Null        # WM_LBUTTONUP
        Write-Output "posted left click at $x,$y"
    }
}

if ($Key -ne 0) {
    $lpDown = [IntPtr](1 -bor ($Scan -shl 16))
    $lpUp = [IntPtr](1 -bor ($Scan -shl 16) -bor 0xC0000000)
    [PostNative]::PostMessage($hwnd, 0x0100, [IntPtr]$Key, $lpDown) | Out-Null    # WM_KEYDOWN
    Start-Sleep -Milliseconds 60
    [PostNative]::PostMessage($hwnd, 0x0101, [IntPtr]$Key, $lpUp) | Out-Null      # WM_KEYUP
    Write-Output ("posted key 0x{0:X} scan 0x{1:X}" -f $Key, $Scan)
}

if ($Wheel -ne 0) {
    $wp = [IntPtr]($Wheel -shl 16)
    $cx = $origin.X + [int]($width / 2); $cy = $origin.Y + [int]($height / 2)
    $lp = [IntPtr](($cy -shl 16) -bor ($cx -band 0xFFFF))
    [PostNative]::PostMessage($hwnd, 0x020A, $wp, $lp) | Out-Null                 # WM_MOUSEWHEEL
    Write-Output "posted wheel $Wheel"
}

Start-Sleep -Milliseconds 300

if ($Out -ne "") {
    Start-Sleep -Milliseconds 300
    $bitmap = New-Object System.Drawing.Bitmap($width, $height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($origin.X, $origin.Y, 0, 0, $bitmap.Size)
    $bitmap.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose(); $bitmap.Dispose()
    Write-Output "saved $Out"
}
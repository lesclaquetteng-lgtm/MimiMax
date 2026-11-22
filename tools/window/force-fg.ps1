param(
    [string]$Hide = "",
    [string]$TitleLike = "*CUBE Launcher*"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FgNative {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
}
"@

if ($Hide -ne "") {
    Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $Hide } |
        ForEach-Object {
            [FgNative]::ShowWindow($_.MainWindowHandle, 6) | Out-Null
            Write-Output ("minimized '" + $_.MainWindowTitle + "'")
        }
    Start-Sleep -Milliseconds 700
}

$target = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitleLike } |
    Select-Object -First 1

if (-not $target) {
    Write-Output "no window matching $TitleLike"
    exit 1
}

if ([FgNative]::IsIconic($target.MainWindowHandle)) {
    [FgNative]::ShowWindow($target.MainWindowHandle, 9) | Out-Null
}

# Windows only lets a background process take the foreground through a keystroke.
[FgNative]::keybd_event(0x12, 0, 0, [IntPtr]::Zero)
[FgNative]::keybd_event(0x12, 0, 2, [IntPtr]::Zero)
Start-Sleep -Milliseconds 120
[FgNative]::ShowWindow($target.MainWindowHandle, 9) | Out-Null
[FgNative]::BringWindowToTop($target.MainWindowHandle) | Out-Null
[FgNative]::SetForegroundWindow($target.MainWindowHandle) | Out-Null
# A game can keep the whole screen in the hit-test stack, so pin our window on top for a moment.
[FgNative]::SetWindowPos($target.MainWindowHandle, [IntPtr](-1), 0, 0, 0, 0, 0x0003) | Out-Null
Start-Sleep -Milliseconds 300
[FgNative]::SetWindowPos($target.MainWindowHandle, [IntPtr](-2), 0, 0, 0, 0, 0x0003) | Out-Null
Start-Sleep -Milliseconds 500

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class FgCheck {
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
}
"@
$fw = [FgCheck]::GetForegroundWindow()
$sb = New-Object System.Text.StringBuilder 512
[FgCheck]::GetWindowText($fw, $sb, 512) | Out-Null
Write-Output ("foreground now: " + $sb.ToString())

$point = New-Object FgNative+POINT
$point.X = 728; $point.Y = 747
$hit = [FgNative]::WindowFromPoint($point)
$owner = 0
[void][FgNative]::GetWindowThreadProcessId($hit, [ref]$owner)
Write-Output ("728,747 owned by: " + (Get-Process -Id $owner -ErrorAction SilentlyContinue).ProcessName)

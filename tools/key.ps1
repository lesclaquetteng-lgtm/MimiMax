param(
    [int]$Vk = 0x0D,
    [int]$Count = 1,
    [string]$TitleLike = "*Minecraft*"
)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Keys {
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@
$p = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like $TitleLike } |
    Select-Object -First 1
if (-not $p) { Write-Output "no window for $TitleLike"; exit 1 }
[Keys]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 300
for ($i = 0; $i -lt $Count; $i++) {
    [Keys]::keybd_event([byte]$Vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [Keys]::keybd_event([byte]$Vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 150
}
Write-Output ("key 0x" + $Vk.ToString("X2") + " x" + $Count)

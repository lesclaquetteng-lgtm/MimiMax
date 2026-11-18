param(
    [string]$Title = 'Minecraft',
    [int]$Vk = 0xA1,
    [int]$Scan = 0,
    [int]$HoldMs = 80
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class KeySend {
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@

$process = Get-Process |
    Where-Object { $_.MainWindowTitle -like "*$Title*" -and $_.MainWindowHandle -ne 0 } |
    Select-Object -First 1

if (-not $process) {
    Write-Error "no window matches $Title"
    exit 1
}

[KeySend]::SetForegroundWindow($process.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 400
[KeySend]::keybd_event([byte]$Vk, [byte]$Scan, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds $HoldMs
[KeySend]::keybd_event([byte]$Vk, [byte]$Scan, 2, [IntPtr]::Zero)
Write-Output ("sent vk 0x{0:X2} to '{1}'" -f $Vk, $process.MainWindowTitle)
param(
	[string]$Title = 'Minecraft'
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FocusWindow {
	[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);
	[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint attach, uint attachTo, bool join);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
	[DllImport("user32.dll")] public static extern IntPtr GetThreadDesktop(uint threadId);
	[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
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
$foreground = [FocusWindow]::GetForegroundWindow()
$foregroundThread = [FocusWindow]::GetWindowThreadProcessId($foreground, [ref]$null)
$currentThread = [FocusWindow]::GetCurrentThreadId()

# Windows refuses SetForegroundWindow to a process that does not own the focus. Attaching the
# input queues of the two threads lifts that restriction for the call.
[FocusWindow]::AttachThreadInput($currentThread, $foregroundThread, $true) | Out-Null
[FocusWindow]::ShowWindow($handle, 9) | Out-Null
[FocusWindow]::BringWindowToTop($handle) | Out-Null
$focused = [FocusWindow]::SetForegroundWindow($handle)
[FocusWindow]::AttachThreadInput($currentThread, $foregroundThread, $false) | Out-Null

Start-Sleep -Milliseconds 900
$now = [FocusWindow]::GetForegroundWindow()
Write-Output "focused=$focused foreground_matches=$($now -eq $handle) pid=$($process.Id) title=$($process.MainWindowTitle)"

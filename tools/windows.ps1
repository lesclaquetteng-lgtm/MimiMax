param(
	[string]$Filter = ''
)

# Lists the top-level windows of the desktop, including the dialogs that have no main window.
# Get-Process only reports MainWindowTitle, which is empty for a MessageBox or a task dialog.

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class WindowList {
	public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

	[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr hWnd);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder name, int count);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);

	public static List<string> All() {
		var rows = new List<string>();
		EnumWindows((handle, parameter) => {
			if (!IsWindowVisible(handle)) return true;

			uint pid;
			GetWindowThreadProcessId(handle, out pid);

			var title = new StringBuilder(512);
			GetWindowText(handle, title, title.Capacity);

			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);

			rows.Add(handle.ToInt64() + "|" + pid + "|" + name + "|" + title.ToString().Replace("\r", " ").Replace("\n", " "));
			return true;
		}, IntPtr.Zero);

		return rows;
	}
}
"@

$rows = [WindowList]::All()

foreach ($row in $rows) {
	$parts = $row.Split('|', 4)
	if ($Filter -and $parts[3] -notlike "*$Filter*") { continue }

	$process = Get-Process -Id $parts[1] -ErrorAction SilentlyContinue
	$name = if ($process) { $process.ProcessName } else { '?' }
	"{0,-10} {1,-14} {2,-18} {3}" -f $parts[0], $name, $parts[2], $parts[3]
}

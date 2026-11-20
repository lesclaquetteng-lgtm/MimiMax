param(
	[string]$Installer = "$PSScriptRoot\..\launcher\desktop\target\release\bundle\nsis\Aether Launcher_1.0.0_x64-setup.exe"
)

# Watches the graphical installer and prints, every second, which page is shown and what the
# installer is doing. Used to tell a slow installer apart from a slow capture script.

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class WatchWindow {
	public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

	[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder name, int count);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

	public static IntPtr Find(uint wanted) {
		var found = IntPtr.Zero;
		EnumWindows((handle, parameter) => {
			if (!IsWindowVisible(handle)) return true;
			uint pid;
			GetWindowThreadProcessId(handle, out pid);
			if (wanted != 0 && pid != wanted) return true;
			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);
			if (name.ToString() != "#32770") return true;
			found = handle;
			return false;
		}, IntPtr.Zero);
		return found;
	}

	public static string Report(IntPtr parent) {
		var text = new StringBuilder();
		EnumChildWindows(parent, (handle, parameter) => {
			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);
			var caption = new StringBuilder(512);
			GetWindowText(handle, caption, caption.Capacity);
			if (caption.Length > 0) {
				text.Append(name).Append('=').Append(caption.ToString()).Append(" | ");
			} else {
				text.Append(name).Append(" | ");
			}
			return true;
		}, IntPtr.Zero);
		return text.ToString();
	}
}
"@

$setup = Start-Process -FilePath $Installer -PassThru
Write-Output "pid=$($setup.Id)"
$clock = [Diagnostics.Stopwatch]::StartNew()
$lastLine = ''

for ($attempt = 0; $attempt -lt 240; $attempt++) {
	Start-Sleep -Milliseconds 500

	if (-not (Get-Process -Id $setup.Id -ErrorAction SilentlyContinue)) {
		Write-Output ("{0,7:N1}s process gone" -f $clock.Elapsed.TotalSeconds)
		break
	}

	$wizard = [WatchWindow]::Find([uint32]$setup.Id)
	if ($wizard -eq [IntPtr]::Zero) { continue }

	$report = [WatchWindow]::Report($wizard)
	if ($report -ne $lastLine) {
		$lastLine = $report
		Write-Output ("{0,7:N1}s {1}" -f $clock.Elapsed.TotalSeconds, $report)
	}

	# Drive the wizard the same way the capture script does.
	if ($report -match 'Button\s+(Fermer|Terminer)') { break }
	if ($report -match 'msctls_progress32') { continue }

	[WatchWindow]::SendMessage($wizard, 0x0111, [IntPtr]1, [IntPtr]0) | Out-Null
}

Write-Output "done at $([math]::Round($clock.Elapsed.TotalSeconds,1))s"

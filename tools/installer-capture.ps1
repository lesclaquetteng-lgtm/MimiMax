param(
	[string]$Installer = "$PSScriptRoot\..\launcher\desktop\target\release\bundle\nsis\Aether Launcher_1.0.0_x64-setup.exe",
	[string]$Shots = "$PSScriptRoot\..\docs\images",
	[string]$Prefix = 'installer',
	[string]$TitleMatch = ''
)

# Runs the installer once and captures every page it shows, in one process.
#
# The polling loop talks to the window through user32 directly: spawning a PowerShell per poll would
# take longer than a page lasts, and pages would be missed.
#
# Usage: powershell -File tools\installer-capture.ps1

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class SetupWindow {
	public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

	[StructLayout(LayoutKind.Sequential)]
	public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

	[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder name, int count);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
	[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);

	public static IntPtr FindWizard(uint wanted) {
		var found = IntPtr.Zero;
		EnumWindows((handle, parameter) => {
			if (!IsWindowVisible(handle)) return true;

			uint pid;
			GetWindowThreadProcessId(handle, out pid);
			if (wanted != 0 && pid != wanted) return true;

			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);
			if (name.ToString() != "#32770") return true;

			var title = new StringBuilder(512);
			GetWindowText(handle, title, title.Capacity);
			if (title.Length == 0) return true;

			found = handle;
			return false;
		}, IntPtr.Zero);

		return found;
	}

	public static string Children(IntPtr parent) {
		var text = new StringBuilder();
		EnumChildWindows(parent, (handle, parameter) => {
			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);

			var caption = new StringBuilder(512);
			GetWindowText(handle, caption, caption.Capacity);

			text.Append(name).Append(' ').Append(caption.ToString()).Append('\n');
			return true;
		}, IntPtr.Zero);

		return text.ToString();
	}
}
"@

New-Item -ItemType Directory -Force $Shots | Out-Null

function Save-Page([IntPtr]$Handle, [string]$Path) {
	$rect = New-Object SetupWindow+RECT
	[SetupWindow]::GetWindowRect($Handle, [ref]$rect) | Out-Null
	$width = $rect.Right - $rect.Left
	$height = $rect.Bottom - $rect.Top
	if ($width -le 0 -or $height -le 0) { return $false }

	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	$hdc = $graphics.GetHdc()
	$printed = [SetupWindow]::PrintWindow($Handle, $hdc, 0x00000002)
	$graphics.ReleaseHdc($hdc)
	$graphics.Dispose()

	if ($printed) { $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png) }
	$bitmap.Dispose()
	return $printed
}

function Get-Page([IntPtr]$Handle) {
	$children = [SetupWindow]::Children($Handle)

	if ($children -match 'RichEdit') { return 'license' }
	if ($children -match 'Button\s+(?:Fermer|Terminer)') { return 'finish' }
	if ($children -match 'msctls_progress32') { return 'install' }
	if ($children -match '\sEdit\s') { return 'folder' }
	if ($children -match 'Suivant') { return 'welcome' }

	return 'unknown'
}

$setup = Start-Process -FilePath $Installer -PassThru
Write-Output "started pid=$($setup.Id)"

# The uninstaller copies itself to the temporary directory and runs there: the window is found by
# its title instead of by the identifier of the process that was started.
$filter = if ($TitleMatch) { [uint32]0 } else { [uint32]$setup.Id }
$last = ''
$idle = 0
$clock = [Diagnostics.Stopwatch]::StartNew()

for ($attempt = 0; $attempt -lt 600; $attempt++) {
	Start-Sleep -Milliseconds 150

	$wizard = [SetupWindow]::FindWizard($filter)
	if ($wizard -eq [IntPtr]::Zero) {
		$idle++
		if ($TitleMatch) {
			if ($idle -gt 20) { break }
		} elseif (-not (Get-Process -Id $setup.Id -ErrorAction SilentlyContinue)) {
			break
		}
		continue
	}

	$idle = 0

	$page = Get-Page $wizard
	if ($page -ne $last) {
		$last = $page
		$shot = Join-Path $Shots "$Prefix-$page.png"
		$saved = Save-Page $wizard $shot
		Write-Output ("{0,7:N1}s page={1} saved={2}" -f $clock.Elapsed.TotalSeconds, $page, $saved)

		# A page is left alone for a moment so the capture has a settled surface.
		Start-Sleep -Milliseconds 500
	}

	if ($page -eq 'finish') { break }

	if ($page -eq 'welcome' -or $page -eq 'license' -or $page -eq 'folder' -or $page -eq 'unknown') {
		[SetupWindow]::SendMessage($wizard, 0x0111, [IntPtr]1, [IntPtr]0) | Out-Null
		Start-Sleep -Milliseconds 400
	}
}

Write-Output "pages=$last elapsed=$([math]::Round($clock.Elapsed.TotalSeconds,1))s"

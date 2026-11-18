param(
	[string]$Action = 'list',
	[long]$Handle = 0,
	[int]$Id = 1,
	[string]$Output = 'window.png',
	[int]$ProcessId = 0
)

# Drives a Windows installer dialog without a mouse: list the dialogs of a process, press one of
# their buttons and capture their surface. Used to verify the NSIS assistant page by page.

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class DialogTool {
	public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

	[StructLayout(LayoutKind.Sequential)]
	public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

	[DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hWnd, StringBuilder name, int count);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
	[DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr lParam);
	[DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hWnd);
	[DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hWnd);

	public class Control {
		public IntPtr Handle;
		public int Id;
		public string ClassName;
		public string Text;
	}

	public static List<Control> Children(IntPtr parent) {
		var rows = new List<Control>();
		EnumChildWindows(parent, (handle, parameter) => {
			var text = new StringBuilder(512);
			GetWindowText(handle, text, text.Capacity);

			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);

			rows.Add(new Control {
				Handle = handle,
				Id = GetDlgCtrlID(handle),
				ClassName = name.ToString(),
				Text = text.ToString().Replace("\r", " ").Replace("\n", " ")
			});
			return true;
		}, IntPtr.Zero);

		return rows;
	}

	public class Row {
		public IntPtr Handle;
		public uint ProcessId;
		public string ClassName;
		public string Title;
	}

	public static List<Row> Dialogs(uint wanted) {
		var rows = new List<Row>();
		EnumWindows((handle, parameter) => {
			if (!IsWindowVisible(handle)) return true;

			uint pid;
			GetWindowThreadProcessId(handle, out pid);
			if (wanted != 0 && pid != wanted) return true;

			var title = new StringBuilder(512);
			GetWindowText(handle, title, title.Capacity);

			var name = new StringBuilder(256);
			GetClassName(handle, name, name.Capacity);

			rows.Add(new Row {
				Handle = handle,
				ProcessId = pid,
				ClassName = name.ToString(),
				Title = title.ToString().Replace("\r", " ").Replace("\n", " ")
			});
			return true;
		}, IntPtr.Zero);

		return rows;
	}
}
"@

switch ($Action) {
	'list' {
		foreach ($row in [DialogTool]::Dialogs([uint32]$ProcessId)) {
			$process = Get-Process -Id $row.ProcessId -ErrorAction SilentlyContinue
			$name = if ($process) { $process.ProcessName } else { '?' }
			"{0,-10} {1,-8} {2,-24} {3}" -f $row.Handle, $name, $row.ClassName, $row.Title
		}
	}

	'click' {
		if ($Handle -eq 0) { Write-Error 'a handle is required'; exit 1 }
		# 0x0111 is WM_COMMAND, 1 is IDOK (Next / Install), 2 is IDCANCEL, 6 is IDYES, 7 is IDNO.
		[DialogTool]::SendMessage([IntPtr]$Handle, 0x0111, [IntPtr]$Id, [IntPtr]0) | Out-Null
		Write-Output "clicked $Id on $Handle"
	}

	'children' {
		if ($Handle -eq 0) { Write-Error 'a handle is required'; exit 1 }
		foreach ($row in [DialogTool]::Children([IntPtr]$Handle)) {
			"{0,-10} id={1,-6} {2,-20} {3}" -f $row.Handle, $row.Id, $row.ClassName, $row.Text
		}
	}

	'press' {
		if ($Handle -eq 0) { Write-Error 'a handle is required'; exit 1 }
		# 0x00F5 is BM_CLICK: presses a button or toggles a check box.
		[DialogTool]::SendMessage([IntPtr]$Handle, 0x00F5, [IntPtr]0, [IntPtr]0) | Out-Null
		Write-Output "pressed $Handle"
	}

	'capture' {
		if ($Handle -eq 0) { Write-Error 'a handle is required'; exit 1 }
		[DialogTool]::ShowWindow([IntPtr]$Handle, 9) | Out-Null
		[DialogTool]::SetForegroundWindow([IntPtr]$Handle) | Out-Null
		Start-Sleep -Milliseconds 600

		$rect = New-Object DialogTool+RECT
		[DialogTool]::GetWindowRect([IntPtr]$Handle, [ref]$rect) | Out-Null
		$width = $rect.Right - $rect.Left
		$height = $rect.Bottom - $rect.Top
		if ($width -le 0 -or $height -le 0) { Write-Error "the window has no surface"; exit 1 }

		$bitmap = New-Object System.Drawing.Bitmap $width, $height
		$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
		$hdc = $graphics.GetHdc()
		# PW_RENDERFULLCONTENT, for the windows that compose through DirectComposition.
		$printed = [DialogTool]::PrintWindow([IntPtr]$Handle, $hdc, 0x00000002)
		$graphics.ReleaseHdc($hdc)
		$graphics.Dispose()

		if (-not $printed) { Write-Error 'the window refused to be printed'; $bitmap.Dispose(); exit 1 }

		$bitmap.Save($Output, [System.Drawing.Imaging.ImageFormat]::Png)
		$bitmap.Dispose()
		Write-Output "captured ${width}x${height} -> $Output"
	}

	default { Write-Error "unknown action: $Action"; exit 1 }
}

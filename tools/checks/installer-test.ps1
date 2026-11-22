param(
	[string]$Installer = "$PSScriptRoot\..\..\launcher\desktop\target\release\bundle\nsis\CUBE Launcher_1.0.0_x64-setup.exe",
	[string]$Shots = "$PSScriptRoot\..\..\docs\images",
	[switch]$NoUninstall
)

# Drives the custom CUBE installer state by state, captures each one and reports what was installed.
#
# The installer is a single custom page (`installer/installer.nsi`), so every button is a
# SS_NOTIFY static: the script finds it by its caption and posts WM_LBUTTONDOWN/WM_LBUTTONUP.
#
# Usage: powershell -File tools\checks\installer-test.ps1 [-NoUninstall]

Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class CubeDialog {
	public delegate bool EnumProc(IntPtr h, IntPtr l);
	[StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
	[DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumProc cb, IntPtr l);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
	[DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
	[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
	[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
	[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
	[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
	[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
	[DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, int m, IntPtr w, IntPtr l);
	[DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, int m, IntPtr w, IntPtr l);
}
"@
Add-Type -AssemblyName System.Drawing

$spawn = [IntPtr]::new(0x00010001)

function Get-VisibleTexts([long]$Handle) {
	$rows = New-Object System.Collections.Generic.List[object]
	[CubeDialog]::EnumChildWindows([IntPtr]$Handle, {
		param($child, $state)
		if ([CubeDialog]::IsWindowVisible($child)) {
			$text = New-Object System.Text.StringBuilder 256
			[CubeDialog]::GetWindowTextW($child, $text, 256) | Out-Null
			$class = New-Object System.Text.StringBuilder 64
			[CubeDialog]::GetClassNameW($child, $class, 64) | Out-Null
			$rows.Add([pscustomobject]@{ Handle = $child; Class = $class.ToString(); Text = $text.ToString() })
		}
		return $true
	}, [IntPtr]::Zero) | Out-Null
	return $rows
}

# The state is named after the title the page draws, not after the click order.
# ASCII-only fragments: PowerShell 5 reads this file as ANSI, so accented literals would not match.
function Get-State([long]$Handle) {
	$texts = (Get-VisibleTexts $Handle | ForEach-Object { $_.Text }) -join "`n"
	if ($texts -match 'TERMINER') { return 'finish' }
	if ($texts -match 'Dossier') { return 'folder' }
	if ($texts -match 'La licence') { return 'license' }
	if ($texts -match 'Installe ton launcher') { return 'welcome' }
	return 'unknown'
}

function Invoke-ClickText([long]$Handle, [string]$Caption) {
	$target = Get-VisibleTexts $Handle | Where-Object { $_.Class -eq 'Static' -and $_.Text -like "*$Caption*" } | Select-Object -First 1
	if (-not $target) { Write-Output "missing=$Caption"; return $false }
	# Posted, not sent: the install page keeps the installer busy for ~1.5 s and a synchronous
	# SendMessage would hide that animation from the state poll below.
	[CubeDialog]::PostMessage($target.Handle, 0x0201, [IntPtr]1, $spawn) | Out-Null
	Start-Sleep -Milliseconds 60
	[CubeDialog]::PostMessage($target.Handle, 0x0202, [IntPtr]0, $spawn) | Out-Null
	Start-Sleep -Milliseconds 150
	return $true
}

function Save-ShotFast([long]$Handle, [string]$Path) {
	$rect = New-Object CubeDialog+RECT
	[CubeDialog]::GetWindowRect([IntPtr]$Handle, [ref]$rect) | Out-Null
	$width = $rect.R - $rect.L
	$height = $rect.B - $rect.T
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	$graphics.CopyFromScreen($rect.L, $rect.T, 0, 0, (New-Object System.Drawing.Size $width, $height))
	$bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
	$graphics.Dispose()
	$bitmap.Dispose()
}

function Save-Shot([long]$Handle, [string]$Path) {
	# A game or the browser keeps stealing the foreground: raise the window, then wait for it.
	[CubeDialog]::ShowWindow([IntPtr]$Handle, 9) | Out-Null
	[CubeDialog]::SetWindowPos([IntPtr]$Handle, [IntPtr](-1), 0, 0, 0, 0, 0x0003) | Out-Null
	[CubeDialog]::SetForegroundWindow([IntPtr]$Handle) | Out-Null
	for ($try = 0; $try -lt 10 -and [CubeDialog]::GetForegroundWindow() -ne [IntPtr]$Handle; $try++) {
		Start-Sleep -Milliseconds 100
		[CubeDialog]::SetForegroundWindow([IntPtr]$Handle) | Out-Null
	}
	Start-Sleep -Milliseconds 500
	$rect = New-Object CubeDialog+RECT
	[CubeDialog]::GetWindowRect([IntPtr]$Handle, [ref]$rect) | Out-Null
	$width = $rect.R - $rect.L
	$height = $rect.B - $rect.T
	$bitmap = New-Object System.Drawing.Bitmap $width, $height
	$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
	$graphics.CopyFromScreen($rect.L, $rect.T, 0, 0, (New-Object System.Drawing.Size $width, $height))
	$bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
	$graphics.Dispose()
	$bitmap.Dispose()
	[CubeDialog]::SetWindowPos([IntPtr]$Handle, [IntPtr](-2), 0, 0, 0, 0, 0x0003) | Out-Null
	Write-Output "shot=$Path"
}

# The frame is custom (no caption), so PrintWindow stays blank: capture the screen instead.
$win = Join-Path $PSScriptRoot '..\window\win.ps1'
$installDir = Join-Path $env:LOCALAPPDATA 'CUBE Launcher'
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\CUBE\CUBE Launcher.lnk'
$desktop = Join-Path ([Environment]::GetFolderPath('Desktop')) 'CUBE Launcher.lnk'

New-Item -ItemType Directory -Force $Shots | Out-Null

if (-not $NoUninstall) {
	Write-Output '=== clean slate ==='
	Get-Process aether-desktop -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
	if (Test-Path (Join-Path $installDir 'uninstall.exe')) {
		Start-Process -FilePath (Join-Path $installDir 'uninstall.exe') -ArgumentList '/S' -Wait
		Start-Sleep -Seconds 4
	}
	Write-Output "installed_before=$(Test-Path (Join-Path $installDir 'aether-desktop.exe'))"
}

$setup = Start-Process -FilePath $Installer -PassThru
$seen = New-Object System.Collections.Generic.List[string]
$names = @{
	welcome = 'installer-1-accueil.png'
	license = 'installer-2-licence.png'
	folder  = 'installer-3-dossier.png'
	progress = 'installer-4-progression.png'
	finish  = 'installer-5-fin.png'
}

$wizard = 0
for ($attempt = 0; $attempt -lt 40 -and -not $wizard; $attempt++) {
	Start-Sleep -Milliseconds 250
	$list = & powershell -NoProfile -ExecutionPolicy Bypass -File $win -Action list 2>$null
	$row = $list | Where-Object { $_ -match 'CUBE Launcher_1' -and $_ -match '#32770' } | Select-Object -First 1
	if ($row) { $wizard = [long]($row -split '\s+')[0] }
}
if (-not $wizard) { Write-Error 'installer window not found'; exit 1 }

$captured = @{}
for ($attempt = 0; $attempt -lt 40; $attempt++) {
	$state = Get-State $wizard
	if ($state -eq 'unknown') {
		# Between "folder" and "finish" the page shows the progress sheet, whose only captions are
		# a percentage and a status line: capture it here.
		if ($seen -contains 'folder' -and -not $captured.ContainsKey('progress')) {
			$captured['progress'] = $true
			$seen.Add('progress')
			Save-ShotFast $wizard (Join-Path $Shots $names['progress'])
		}
		Start-Sleep -Milliseconds 300
		continue
	}

	if (-not $captured.ContainsKey($state)) {
		$captured[$state] = $true
		$seen.Add($state)
		Save-Shot $wizard (Join-Path $Shots $names[$state])
	}

	switch ($state) {
		'welcome' { Invoke-ClickText $wizard 'INSTALLER' | Out-Null }
		'license' {
			$box = Get-VisibleTexts $wizard | Where-Object { $_.Text -like '*J''accepte*' } | Select-Object -First 1
			if ($box -and $box.Text.TrimStart().StartsWith([char]0x2610)) {
				Invoke-ClickText $wizard "J'accepte" | Out-Null
			} else {
				Invoke-ClickText $wizard 'CONTINUER' | Out-Null
			}
		}
		'folder' { Invoke-ClickText $wizard 'INSTALLER' | Out-Null }
		'finish' { break }
	}
	if ($state -eq 'finish') { break }
	Start-Sleep -Milliseconds 400
}

Write-Output "states=$($seen -join ' -> ')"

# Leave through TERMINER so a checked "Lancer CUBE Launcher" never starts a second instance.
if ((Get-State $wizard) -eq 'finish') { Invoke-ClickText $wizard 'TERMINER' | Out-Null }
Start-Sleep -Seconds 3
if (Get-Process -Id $setup.Id -ErrorAction SilentlyContinue) { Stop-Process -Id $setup.Id -Force -ErrorAction SilentlyContinue }

Write-Output '=== installed layout ==='
Get-ChildItem $installDir -ErrorAction SilentlyContinue | Select-Object Name, Length | Format-Table -AutoSize
Write-Output "desktop_shortcut=$(Test-Path $desktop)"
Write-Output "start_menu_link=$(Test-Path $startMenu)"
$entry = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
	Where-Object { $_.DisplayName -like 'CUBE*' } | Select-Object -First 1
Write-Output "uninstall_entry=$($entry.DisplayName) $($entry.DisplayVersion)"

if (-not $NoUninstall -and (Test-Path (Join-Path $installDir 'uninstall.exe'))) {
	Write-Output '=== silent uninstall ==='
	Get-Process aether-desktop -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
	$uninstall = Start-Process -FilePath (Join-Path $installDir 'uninstall.exe') -ArgumentList '/S' -PassThru -Wait
	Start-Sleep -Seconds 5
	Write-Output "uninstall_exit=$($uninstall.ExitCode)"
	Write-Output "folder_gone=$(-not (Test-Path $installDir))"
	Write-Output "desktop_shortcut_gone=$(-not (Test-Path $desktop))"
	Write-Output "start_menu_gone=$(-not (Test-Path (Split-Path $startMenu -Parent)))"
	$entry = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
		Where-Object { $_.DisplayName -like 'CUBE*' }
	Write-Output "uninstall_entry_gone=$(-not $entry)"
}

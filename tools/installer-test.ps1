param(
	[string]$Installer = "$PSScriptRoot\..\launcher\desktop\target\release\bundle\nsis\Aether Launcher_1.0.0_x64-setup.exe",
	[string]$Shots = "$PSScriptRoot\..\docs\images",
	[switch]$NoUninstall
)

# Drives the assistant page by page, captures each page and reports the layout it installed.
#
# Usage: powershell -File tools\installer-test.ps1 [-NoUninstall]

$win = Join-Path $PSScriptRoot 'win.ps1'
New-Item -ItemType Directory -Force $Shots | Out-Null

function Invoke-Win {
	param([string]$Action, [long]$Handle = 0, [int]$Id = 1, [string]$Output = '', [int]$ProcessId = 0)

	$arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $win, '-Action', $Action)
	if ($Handle) { $arguments += @('-Handle', $Handle) }
	if ($Id -ne 1) { $arguments += @('-Id', $Id) }
	if ($Output) { $arguments += @('-Output', $Output) }
	if ($ProcessId) { $arguments += @('-ProcessId', $ProcessId) }

	& powershell @arguments 2>$null
}

function Get-Wizard([int]$ProcessId) {
	$rows = Invoke-Win -Action list -ProcessId $ProcessId | Where-Object { $_ -match '\s#32770\s' }
	if (-not $rows) { return 0 }
	[long](($rows | Select-Object -First 1) -split '\s+')[0]
}

# The page is recognised by the controls it contains, not by the order of the clicks.
function Get-Page([long]$Handle) {
	$children = (Invoke-Win -Action children -Handle $Handle) -join "`n"

	if ($children -match 'RichEdit') { return 'license' }
	if ($children -match 'Button\s+.*Terminer') { return 'finish' }
	if ($children -match 'msctls_progress32') { return 'install' }
	if ($children -match '\sEdit\s') { return 'folder' }
	if ($children -match 'Suivant') { return 'welcome' }

	return 'unknown'
}

if (-not $NoUninstall) {
	Write-Output '=== clean slate ==='
	Get-Process aether-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
	$uninstaller = Join-Path $env:LOCALAPPDATA 'Aether Launcher\uninstall.exe'
	if (Test-Path $uninstaller) {
		Start-Process -FilePath $uninstaller -ArgumentList '/S' -Wait
		Start-Sleep -Seconds 3
	}
	Write-Output "installed_before=$(Test-Path (Join-Path $env:LOCALAPPDATA 'Aether Launcher\aether-desktop.exe'))"
}

$setup = Start-Process -FilePath $Installer -PassThru
$seen = New-Object System.Collections.Generic.List[string]
$last = ''

for ($attempt = 0; $attempt -lt 90; $attempt++) {
	Start-Sleep -Milliseconds 500

	$wizard = Get-Wizard $setup.Id
	if (-not $wizard) {
		if (-not (Get-Process -Id $setup.Id -ErrorAction SilentlyContinue)) { break }
		continue
	}

	$page = Get-Page $wizard
	if ($page -ne $last) {
		$last = $page
		$seen.Add($page)
		$shot = Join-Path $Shots "installer-$page.png"
		$capture = Invoke-Win -Action capture -Handle $wizard -Output $shot
		Write-Output "page=$page $(($capture | Select-Object -Last 1))"
	}

	switch ($page) {
		'welcome' { Invoke-Win -Action click -Handle $wizard -Id 1 | Out-Null; Start-Sleep -Seconds 1 }
		'license' { Invoke-Win -Action click -Handle $wizard -Id 1 | Out-Null; Start-Sleep -Seconds 1 }
		'folder' { Invoke-Win -Action click -Handle $wizard -Id 1 | Out-Null; Start-Sleep -Seconds 1 }
		'finish' { break }
		default { }
	}

	if ($page -eq 'finish') { break }
}

Start-Sleep -Seconds 2
$wizard = Get-Wizard $setup.Id
if ($wizard) {
	# The finish page has a check box that launches the application and a "Terminer" button.
	Invoke-Win -Action click -Handle $wizard -Id 1 | Out-Null
}

Start-Sleep -Seconds 5

Write-Output "pages=$($seen -join ' -> ')"
Write-Output '=== installed layout ==='
$target = Join-Path $env:LOCALAPPDATA 'Aether Launcher'
Get-ChildItem $target -ErrorAction SilentlyContinue | Select-Object Name, Length | Format-Table -AutoSize
Write-Output "desktop_shortcut=$(Test-Path (Join-Path $env:USERPROFILE 'Desktop\Aether Launcher.lnk'))"
Write-Output "start_menu_link=$(Test-Path (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Aether\Aether Launcher.lnk'))"
Write-Output "app_running=$((Get-Process aether-desktop -ErrorAction SilentlyContinue | Measure-Object).Count)"

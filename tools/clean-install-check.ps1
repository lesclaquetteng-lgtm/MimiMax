param(
	[string]$Exe = "$PSScriptRoot\..\launcher\desktop\target\release\aether-desktop.exe",
	[int]$Port = 9345
)

# Proves that a launcher which only has its own executable can install the game.
#
# The core looks for the client jar next to itself, in the project and around the home. On a machine
# that only received the portable executable or the installer, none of those exists: the shell has to
# carry the archive itself. This check runs from an empty folder, with an empty home, no
# AETHER_MOD_JAR and no sources in sight, and watches the installation get past the point where it
# used to answer "aether-client/build/libs/aether-client-1.0.0.jar is missing".
#
# Usage: powershell -File tools\clean-install-check.ps1 [-Exe <path>]

$ErrorActionPreference = 'Stop'

$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$core = Join-Path $root 'launcher\target\release\aether-launcher.exe'
$jar = Join-Path $root 'aether-client\build\libs\aether-client-1.0.0.jar'
$home_ = Join-Path $env:TEMP 'aether-clean-home'
$mods = Join-Path $env:LOCALAPPDATA 'dev.aether.launcher\mods'
$mod = Join-Path $mods 'aether-client.jar'

Get-Process aether-desktop -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1

Remove-Item $home_ -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $home_ | Out-Null
Remove-Item $mods -Recurse -Force -ErrorAction SilentlyContinue

$env:AETHER_HOME = $home_
Remove-Item Env:\AETHER_MOD_JAR -ErrorAction SilentlyContinue

# Control: the core is copied alone into the empty folder, so that it can only find a jar through
# AETHER_MOD_JAR. Started by hand, without that variable, it reports the missing archive.
$controlCore = Join-Path $home_ 'aether-launcher.exe'
Copy-Item $core $controlCore -Force
$controlOut = Join-Path $home_ 'control.ndjson'
$controlErr = Join-Path $home_ 'control.err'
Start-Process -FilePath $controlCore -ArgumentList 'install' -WorkingDirectory $home_ `
	-RedirectStandardOutput $controlOut -RedirectStandardError $controlErr -Wait -WindowStyle Hidden | Out-Null
$control = ((Get-Content $controlOut -ErrorAction SilentlyContinue) -join ' ')
$controlMissing = $control -match 'jar was not found|is missing'
Write-Output "control=$control"
Write-Output "control_reports_missing_mod=$controlMissing"
Remove-Item $controlCore, $controlOut, $controlErr -Force -ErrorAction SilentlyContinue

# The shell extracts the embedded archive before it runs anything.
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$shell = Start-Process -FilePath ([IO.Path]::GetFullPath($Exe)) -WorkingDirectory $home_ -PassThru

$ready = $false
for ($attempt = 0; $attempt -lt 30; $attempt++) {
	Start-Sleep -Seconds 1
	try {
		$response = Invoke-WebRequest "http://127.0.0.1:$Port/json/list" -UseBasicParsing -TimeoutSec 2
		if ($response.StatusCode -eq 200) {
			$ready = $true
			break
		}
	} catch {
	}
}

if (-not $ready) {
	Write-Output 'shell_cdp_ready=false'
	Stop-Process -Id $shell.Id -Force -ErrorAction SilentlyContinue
	exit 1
}

# A command through the bridge makes the shell run the core, which is what extracts the mod.
$bridge = & node "$PSScriptRoot\cdp.mjs" $Port "window.aether.info().then(r=>JSON.stringify({ok:r.ok}))" 2>&1
Write-Output "bridge=$bridge"

for ($attempt = 0; $attempt -lt 20; $attempt++) {
	if (Test-Path -LiteralPath $mod) { break }
	Start-Sleep -Milliseconds 500
}

$expected = (Get-FileHash $jar -Algorithm SHA256).Hash
$actual = if (Test-Path -LiteralPath $mod) { (Get-FileHash $mod -Algorithm SHA256).Hash } else { '' }
Write-Output "mod_extracted=$([bool]$actual) mod_hash_matches=$($actual -eq $expected) bytes=$(if (Test-Path $mod) { (Get-Item $mod).Length } else { 0 })"

# Start an installation and watch its first events: they only come after the jar has been found.
$start = & node "$PSScriptRoot\cdp.mjs" $Port "window.__events=[]; window.aether.onInstallProgress(e=>window.__events.push(e)); window.__install=window.aether.install({}).then(r=>window.__installResult=r); 'started'" --no-await 2>&1
Write-Output "install_started=$start"

for ($attempt = 0; $attempt -lt 20; $attempt++) {
	Start-Sleep -Seconds 1
	$state = & node "$PSScriptRoot\cdp.mjs" $Port "JSON.stringify({events: window.__events.slice(0, 2), result: window.__installResult || null})" 2>&1
	if ($state -match '"events":\[\{') {
		break
	}
	if ($state -match '"result":\{') {
		break
	}
}
Write-Output "install_state=$state"

& node "$PSScriptRoot\cdp.mjs" $Port "window.aether.installStop()" 2>&1 | Out-Null
Start-Sleep -Seconds 2

Stop-Process -Id $shell.Id -Force -ErrorAction SilentlyContinue
Get-Process java -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item $home_ -Recurse -Force -ErrorAction SilentlyContinue

$failedOnMod = $state -match 'jar was not found|aether-client/build/libs'
$progressed = $state -match '"events":\[\{'
Write-Output "install_failed_on_mod=$failedOnMod install_progressed=$progressed"

if ($controlMissing -and $actual -eq $expected -and $progressed -and -not $failedOnMod) {
	Write-Output 'VERDICT: a clean machine can install the game'
	exit 0
}

Write-Output 'VERDICT: FAILED'
exit 1

# Replays every end-to-end check of tools/e2e/ against the real shell, one window per scenario.
#
# Each check needs its own core mode and its own data home, so the window is started fresh each time:
#   powershell -NoProfile -File tools/e2e/replay.ps1
param([string]$Root = "C:\Users\ee\Desktop\MimiMax", [string]$Only = '')

$Shell = "$Root\launcher\desktop\target\release\aether-desktop.exe"
$Fake = "$Root\tools\e2e\fake-core.exe"
$Hang = "$Root\tools\e2e\hang.exe"
$Real = "$Root\launcher\target\release\aether-launcher.exe"
$PidFile = "$Root\tools\e2e\pids-replay.txt"

$cases = @(
    @{ name = 'a11y';     port = 9222; script = 'a11y.mjs';         args = @();           fail = '';                    bin = $Fake },
    @{ name = 'sliders';  port = 9223; script = 'sliders.mjs';      args = @();           fail = '';                    bin = $Fake },
    @{ name = 'screens';  port = 9224; script = 'screens.mjs';      args = @("$Root\tools\e2e\shots-replay"); fail = ''; bin = $Fake; seed = 'account' },
    @{ name = 'drive';    port = 9225; script = 'drive.mjs';        args = @($PidFile);   fail = '';                    bin = $Fake; seed = 'account' },
    @{ name = 'profiles'; port = 9226; script = 'profiles.mjs';     args = @();           fail = '';                    bin = $Real; seed = 'hardcore' },
    @{ name = 'failure';  port = 9227; script = 'failure.mjs';      args = @('documents'); fail = 'install,launch'; bin = $Fake; seed = 'account' },
    @{ name = 'crash';    port = 9228; script = 'crash-report.mjs'; args = @('game');     fail = 'launch-exit';         bin = $Fake; seed = 'account' },
    @{ name = 'cancel';   port = 9229; script = 'cancel.mjs';       args = @();           fail = 'install-slow';        bin = $Fake; seed = 'account' },
    @{ name = 'stubborn'; port = 9230; script = 'stubborn.mjs';     args = @();           fail = 'install-stubborn';    bin = $Fake; seed = 'account' },
    @{ name = 'timeout';  port = 9231; script = 'timeout.mjs';      args = @();           fail = '';                    bin = $Hang },
    @{ name = 'hit';      port = 9232; script = 'hit.mjs';          args = @();           fail = '';                    bin = $Fake; seed = 'account' }
)

$stamp = Get-Random
# A window left open by an earlier run would answer on a fixed port and steal the checks.
$portOffset = (Get-Random -Maximum 9) * 100

foreach ($case in $cases | Where-Object { -not $Only -or $Only.Split(',') -contains $_.name }) {
    # A fresh home each time: a leftover settings file or account list would change the checks.
    $viewport = $null
    $dataHome = "$Root\tools\e2e\home-replay-$($case.name)-$stamp"
    # Same rule as tools/e2e/guard.mjs, checked before anything is deleted.
    if ($dataHome -notlike "$Root\tools\e2e\home-replay-*") {
        throw "refused: $dataHome is not a throwaway home under tools/e2e"
    }
    New-Item -ItemType Directory -Path $dataHome -Force | Out-Null

    if (Test-Path $PidFile) { Remove-Item -Force $PidFile }

    $env:AETHER_HOME = $dataHome
    $env:AETHER_LAUNCHER_BIN = $case.bin
    # The fake core only fakes install and launch; it forwards accounts and profiles to the real one.
    $env:AETHER_REAL_CORE = $Real
    $env:AETHER_FAKE_FAIL = $case.fail
    $env:AETHER_FAKE_PID_FILE = $PidFile
    $port = $case.port + $portOffset
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$port"

    # The checks need a player: an account to list, select and play with, mods to list and toggle,
    # and a finished session on disk.
    if ($case.seed) {
        if ($case.seed -ne 'hardcore') { & $Real account add-offline "Tester" | Out-Null }

        $mods = "$dataHome\.minecraft\mods"
        New-Item -ItemType Directory -Path $mods -Force | Out-Null
        Set-Content "$mods\aether-client.jar" "dummy aether"
        Set-Content "$mods\sodium.jar" "dummy sodium"
        Set-Content "$mods\iris.jar.disabled" "dummy iris"

        $logs = "$dataHome\aether\logs"
        New-Item -ItemType Directory -Path $logs -Force | Out-Null
        Set-Content "$logs\session-20261118-160000.log" @(
            "# Aether session 2026-11-18 16:00:00",
            "fake java started",
            "fake java child 4242",
            "# exit code 0"
        )
    }

    if ($case.seed -eq 'hardcore') {
        & $Real profiles add "Hardcore" --version "1.20.1" --loader "0.20.1" --memory 8192 | Out-Null
    }

    $process = Start-Process -FilePath $Shell -WorkingDirectory $Root -PassThru
    $ready = $false
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 400
        try {
            $answer = Invoke-RestMethod "http://127.0.0.1:$port/json/list" -TimeoutSec 2
            if ($answer.Count -gt 0) { $ready = $true; break }
        } catch {}
    }

    Write-Host "=== $($case.name) : port $port, home $dataHome, devtools $ready ==="
    if ($ready) {
        # The endpoint answers before the interface has bound its handlers: let the boot finish.
        # No viewport override here: the checks scroll their own target into view before clicking, so
        # they run in the window the player actually gets (1180x760).
        Start-Sleep -Seconds 3
        $arguments = @($port.ToString()) + $case.args
        & node "$Root\tools\e2e\$($case.script)" @arguments
        Write-Host "EXIT $($case.name)=$LASTEXITCODE"
    } else {
        Write-Host "EXIT $($case.name)=no-devtools-endpoint"
    }

    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Milliseconds 600
}

$env:AETHER_FAKE_FAIL = ''
$env:AETHER_FAKE_PID_FILE = ''

Get-ChildItem "$Root\tools\e2e" -Filter 'home-replay-*' -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -like "*-$stamp" } |
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue

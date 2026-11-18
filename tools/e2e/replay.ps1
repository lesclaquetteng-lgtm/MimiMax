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
    @{ name = 'failure';  port = 9227; script = 'failure.mjs';      args = @('documents'); fail = 'install,launch-exit'; bin = $Fake; seed = 'account' },
    @{ name = 'crash';    port = 9228; script = 'crash-report.mjs'; args = @('game');     fail = 'launch-exit';         bin = $Fake; seed = 'account' },
    @{ name = 'cancel';   port = 9229; script = 'cancel.mjs';       args = @();           fail = 'install-slow';        bin = $Fake; seed = 'account' },
    @{ name = 'stubborn'; port = 9230; script = 'stubborn.mjs';     args = @();           fail = 'install-stubborn';    bin = $Fake; seed = 'account' },
    @{ name = 'timeout';  port = 9231; script = 'timeout.mjs';      args = @();           fail = '';                    bin = $Hang }
)

foreach ($case in $cases | Where-Object { -not $Only -or $Only.Split(',') -contains $_.name }) {
    $home = "$Root\tools\e2e\home-replay-$($case.name)"
    if (Test-Path $home) { Remove-Item -Recurse -Force $home }
    New-Item -ItemType Directory -Path $home -Force | Out-Null

    if (Test-Path $PidFile) { Remove-Item -Force $PidFile }

    $env:AETHER_HOME = $home
    $env:AETHER_LAUNCHER_BIN = $case.bin
    # The fake core only fakes install and launch; it forwards accounts and profiles to the real one.
    $env:AETHER_REAL_CORE = $Real
    $env:AETHER_FAKE_FAIL = $case.fail
    $env:AETHER_FAKE_PID_FILE = $PidFile
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$($case.port)"

    # The checks need a player: an account to list, select and play with.
    if ($case.seed -and $case.seed -ne 'hardcore') { & $Real account add-offline "Tester" | Out-Null }
    if ($case.seed -eq 'hardcore') {
        & $Real profiles add "Hardcore" --version "1.20.1" --loader "0.20.1" --memory 8192 | Out-Null
    }

    $process = Start-Process -FilePath $Shell -WorkingDirectory $Root -PassThru
    $ready = $false
    for ($i = 0; $i -lt 40; $i++) {
        Start-Sleep -Milliseconds 400
        try {
            $answer = Invoke-RestMethod "http://127.0.0.1:$($case.port)/json/list" -TimeoutSec 2
            if ($answer.Count -gt 0) { $ready = $true; break }
        } catch {}
    }

    Write-Host "=== $($case.name) : port $($case.port), devtools $ready ==="
    if ($ready) {
        $arguments = @($case.port.ToString()) + $case.args
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

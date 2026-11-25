param(
    [int]$Port = 9233,
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$Root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$TempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$HomeDir = Join-Path $TempRoot ("aether-ui-home-" + [guid]::NewGuid().ToString("N"))
$WebViewDataDir = Join-Path $HomeDir "webview2-user-data"
if (-not $OutDir) {
    $OutDir = Join-Path $TempRoot "aether-ui-shots"
}
$OutDir = [System.IO.Path]::GetFullPath($OutDir)
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $HomeDir ".minecraft\mods") -Force | Out-Null
New-Item -ItemType Directory -Path $WebViewDataDir -Force | Out-Null

$Core = Join-Path $Root "launcher\target\release\aether-launcher.exe"
if (-not (Test-Path $Core)) {
    $Core = Join-Path $Root "launcher\target\debug\aether-launcher.exe"
}
if (-not (Test-Path $Core)) {
    throw "Launcher core not built: $Core"
}

$Shell = Join-Path $Root "launcher\desktop\target\release\aether-desktop.exe"
if (-not (Test-Path $Shell)) {
    throw "Native shell not built: $Shell"
}

# All fixtures and the WebView2 profile live in a fresh runner-temp home. A unique user-data
# folder prevents another WebView2 host from reusing a browser process with debugging disabled.
Set-Content (Join-Path $HomeDir ".minecraft\mods\aether-client.jar") "dummy aether"
Set-Content (Join-Path $HomeDir ".minecraft\mods\sodium.jar") "dummy sodium"
Set-Content (Join-Path $HomeDir ".minecraft\mods\iris.jar.disabled") "dummy iris"

$env:AETHER_HOME = $HomeDir
& $Core account add-offline "AlphaTester" | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Could not seed the offline test account (exit code $LASTEXITCODE)"
}
& $Core profiles add "Competitive" --version "1.21.11" --loader "0.20.1" --memory 6144 | Out-Null
if ($LASTEXITCODE -ne 0) {
    throw "Could not seed the test profile (exit code $LASTEXITCODE)"
}

$env:AETHER_LAUNCHER_BIN = $Core
$env:WEBVIEW2_USER_DATA_FOLDER = $WebViewDataDir
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port --remote-allow-origins=*"
$Process = $null
$Http = [System.Net.Http.HttpClient]::new()
$Http.Timeout = [TimeSpan]::FromMilliseconds(1000)

try {
    $Process = Start-Process -FilePath $Shell -WorkingDirectory $Root -PassThru
    Write-Host "Started native shell PID $($Process.Id) with an isolated WebView2 profile and debugging on port $Port"

    $Ready = $false
    $LastProbeError = "no response"
    for ($Attempt = 0; $Attempt -lt 60; $Attempt++) {
        if ($Process.HasExited) {
            $Process.Refresh()
            throw "Native shell exited before WebView2 became ready (exit code $($Process.ExitCode))"
        }

        try {
            $Json = $Http.GetStringAsync("http://127.0.0.1:$Port/json/list").GetAwaiter().GetResult()
            $Targets = ConvertFrom-Json -InputObject $Json
            if (@($Targets).Count -gt 0) {
                $Ready = $true
                break
            }
            $LastProbeError = "debug endpoint returned an empty target list"
        } catch {
            $LastProbeError = $_.Exception.Message
        }

        Start-Sleep -Milliseconds 500
    }

    if (-not $Ready) {
        $ShellState = if ($Process.HasExited) {
            $Process.Refresh()
            "exited with code $($Process.ExitCode)"
        } else {
            "still running (PID $($Process.Id))"
        }

        Write-Host "WebView2 startup diagnostics: shell=$ShellState; userDataFolder=$WebViewDataDir; lastProbe=$LastProbeError"

        try {
            $WebViewProcesses = @(Get-CimInstance -ClassName Win32_Process -Filter "Name='msedgewebview2.exe'" |
                Select-Object ProcessId, ParentProcessId, ExecutablePath, CommandLine)
            if ($WebViewProcesses.Count -gt 0) {
                Write-Host "WebView2 processes:"
                $WebViewProcesses | Format-List | Out-String | Write-Host
            } else {
                Write-Host "No msedgewebview2.exe process was found."
            }
        } catch {
            Write-Host "Could not inspect WebView2 processes: $($_.Exception.Message)"
        }

        try {
            $Listeners = @(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
                Select-Object LocalAddress, LocalPort, OwningProcess)
            if ($Listeners.Count -gt 0) {
                Write-Host "Listeners on the debugging port:"
                $Listeners | Format-Table -AutoSize | Out-String | Write-Host
            } else {
                Write-Host "No process is listening on debugging port $Port."
            }
        } catch {
            Write-Host "Could not inspect the debugging port: $($_.Exception.Message)"
        }

        throw "WebView2 debugging endpoint did not become ready on port $Port within 30 seconds; $LastProbeError"
    }

    Write-Host "WebView2 debugging endpoint is ready; running real-window UI checks and capturing screenshots in $OutDir"
    Push-Location $Root
    try {
        node (Join-Path $Root "tools\checks\ui-runner.mjs") $Port $OutDir
        if ($LASTEXITCODE -ne 0) {
            throw "UI checks failed with exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }
} finally {
    if ($Process -and -not $Process.HasExited) {
        Write-Host "Closing test shell PID $($Process.Id)"
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
    }
    $Http.Dispose()
    Remove-Item Env:\AETHER_HOME -ErrorAction SilentlyContinue
    Remove-Item Env:\AETHER_LAUNCHER_BIN -ErrorAction SilentlyContinue
    Remove-Item Env:\WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $HomeDir -Recurse -Force -ErrorAction SilentlyContinue
}

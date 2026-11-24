param(
    [int]$Port = 9233,
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"
$Root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$TempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$HomeDir = Join-Path $TempRoot ("aether-ui-home-" + [guid]::NewGuid().ToString("N"))
if (-not $OutDir) {
    $OutDir = Join-Path $TempRoot "aether-ui-shots"
}
$OutDir = [System.IO.Path]::GetFullPath($OutDir)
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $HomeDir ".minecraft\mods") -Force | Out-Null

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

# All fixtures live in a fresh runner-temp home; no developer profile or repository data is touched.
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
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"
$Process = $null

try {
    $Process = Start-Process -FilePath $Shell -WorkingDirectory $Root -PassThru
    Write-Host "Started native shell PID $($Process.Id) with WebView2 debugging on port $Port"

    $Ready = $false
    for ($Attempt = 0; $Attempt -lt 40; $Attempt++) {
        Start-Sleep -Milliseconds 500
        try {
            $Response = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/list" -TimeoutSec 2
            if ($Response.Count -gt 0) {
                $Ready = $true
                break
            }
        } catch {
            # The WebView2 renderer is still starting.
        }
    }

    if (-not $Ready) {
        throw "WebView2 debugging endpoint did not respond on port $Port"
    }

    Write-Host "Running real-window UI checks and capturing screenshots in $OutDir"
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
    Remove-Item Env:\AETHER_HOME -ErrorAction SilentlyContinue
    Remove-Item Env:\AETHER_LAUNCHER_BIN -ErrorAction SilentlyContinue
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}

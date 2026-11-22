param(
    [int]$Port = 9233,
    [string]$OutDir = "C:\Users\ee\Desktop\MimiMax\tools\new-ui-shots"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$Root = "C:\Users\ee\Desktop\MimiMax"
$HomeDir = "$Root\tools\test-home"
if (Test-Path $HomeDir) { Remove-Item -Recurse -Force $HomeDir }
New-Item -ItemType Directory -Path "$HomeDir\.minecraft\mods" -Force | Out-Null

# Seed test files
Set-Content "$HomeDir\.minecraft\mods\aether-client.jar" "dummy aether"
Set-Content "$HomeDir\.minecraft\mods\sodium.jar" "dummy sodium"
Set-Content "$HomeDir\.minecraft\mods\iris.jar.disabled" "dummy iris"

# Add an offline account
$Core = "$Root\launcher\target\release\aether-launcher.exe"
if (-not (Test-Path $Core)) {
    $Core = "$Root\launcher\target\debug\aether-launcher.exe"
}
$env:AETHER_HOME = $HomeDir
& $Core account add-offline "AlphaTester" | Out-Null
& $Core profiles add "Competitive" --version "1.21.1" --loader "0.16.9" --memory 6144 | Out-Null

Write-Host "Seeded test home at $HomeDir"

# The shell is the native Tauri one; WebView2 takes its debugging port from the environment.
$env:AETHER_HOME = $HomeDir
$env:AETHER_LAUNCHER_BIN = $Core
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port"

$shellPath = "$Root\launcher\desktop\target\release\aether-desktop.exe"
if (-not (Test-Path $shellPath)) {
    throw "Shell not built: cargo build --release --manifest-path launcher/desktop/Cargo.toml"
}

$proc = Start-Process -FilePath $shellPath -PassThru

Write-Host "Started the shell PID $($proc.Id) with debugging on port $Port"

try {
    # Wait for DevTools port
    $ready = $false
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Milliseconds 500
        try {
            $resp = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/list" -TimeoutSec 2
            if ($resp.Count -gt 0) { $ready = $true; break }
        } catch {}
    }

    if (-not $ready) {
        throw "WebView2 debugging endpoint did not respond on port $Port"
    }

    Write-Host "DevTools endpoint ready. Running UI test runner..."
    node "$Root\tools\checks\ui-runner.mjs" $Port $OutDir

} finally {
    Write-Host "Terminating the shell PID $($proc.Id)..."
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    # The core of the shell is a child of it, and it may have started the game.
    Get-Process aether-launcher -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

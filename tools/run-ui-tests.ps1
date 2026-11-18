param(
    [int]$Port = 9233,
    [string]$OutDir = "C:\Users\ee\Desktop\MimiMax\tools\new-ui-shots"
)

$ErrorActionPreference = "Stop"
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null

$HomeDir = "C:\Users\ee\Desktop\MimiMax\tools\test-home"
if (Test-Path $HomeDir) { Remove-Item -Recurse -Force $HomeDir }
New-Item -ItemType Directory -Path "$HomeDir\.minecraft\mods" -Force | Out-Null

# Seed test files
Set-Content "$HomeDir\.minecraft\mods\aether-client.jar" "dummy aether"
Set-Content "$HomeDir\.minecraft\mods\sodium.jar" "dummy sodium"
Set-Content "$HomeDir\.minecraft\mods\iris.jar.disabled" "dummy iris"

# Add an offline account
$Core = "C:\Users\ee\Desktop\MimiMax\launcher\target\release\aether-launcher.exe"
if (-not (Test-Path $Core)) {
    $Core = "C:\Users\ee\Desktop\MimiMax\launcher\target\debug\aether-launcher.exe"
}
$env:AETHER_HOME = $HomeDir
& $Core account add-offline "AlphaTester" | Out-Null
& $Core profiles add "Competitive" --version "1.21.1" --loader "0.16.9" --memory 6144 | Out-Null

Write-Host "Seeded test home at $HomeDir"

# Start Electron
$env:ELECTRON_RUN_AS_NODE = $null
[Environment]::SetEnvironmentVariable("ELECTRON_RUN_AS_NODE", $null, "Process")
$env:AETHER_HOME = $HomeDir
$env:AETHER_LAUNCHER_BIN = $Core
$electronPath = (Get-Item "C:\Users\ee\Desktop\MimiMax\launcher\node_modules\electron\dist\electron.exe").FullName

$proc = Start-Process -FilePath $electronPath `
    -ArgumentList "C:\Users\ee\Desktop\MimiMax\launcher", "--remote-debugging-port=$Port" `
    -PassThru

Write-Host "Started Electron PID $($proc.Id) with debugging on port $Port"

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
        throw "Electron debugging endpoint did not respond on port $Port"
    }

    Write-Host "DevTools endpoint ready. Running UI test runner..."
    node "C:\Users\ee\Desktop\MimiMax\tools\ui-runner.mjs" $Port $OutDir

} finally {
    Write-Host "Terminating Electron PID $($proc.Id)..."
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Get-Process electron -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like "*MimiMax*" } | Stop-Process -Force -ErrorAction SilentlyContinue
}

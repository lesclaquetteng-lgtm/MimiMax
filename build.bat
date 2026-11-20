@echo off
setlocal
cd /d "%~dp0"

echo ========================================================
echo   Compilation MimiMax / Aether Launcher
echo ========================================================

echo.
echo [1/4] Compilation du Core Rust...
pushd launcher
call cargo build --release
if errorlevel 1 (
    echo [ERREUR] Echec de compilation du Core Rust.
    popd
    pause
    exit /b 1
)
popd

echo.
echo [2/4] Generation du Mod Minecraft (Fabric)...
pushd aether-client
call gradlew.bat --no-daemon build
if errorlevel 1 (
    echo [ERREUR] Echec de compilation du mod Fabric.
    echo La coquille native embarque ce JAR : elle ne peut pas etre compilee sans lui.
    popd
    pause
    exit /b 1
)
if not exist "..\tools\aether-home\.minecraft\mods" mkdir "..\tools\aether-home\.minecraft\mods"
copy /Y "build\libs\aether-client-1.0.0.jar" "..\tools\aether-home\.minecraft\mods\aether-client.jar" >nul
if errorlevel 1 (
    echo [ERREUR] Impossible de copier le JAR du client.
    popd
    pause
    exit /b 1
)
popd

echo.
echo [3/4] Compilation de la coquille native (Tauri)...
pushd launcher
call cargo build --release --manifest-path desktop\Cargo.toml
if errorlevel 1 (
    echo [ERREUR] Echec de compilation de la coquille Tauri.
    popd
    pause
    exit /b 1
)
popd

echo.
echo [4/4] Creation des executables racines (.exe)...
rustc -O -C opt-level=z -C lto=yes -C strip=symbols tools\launcher-runner.rs -o MimiMax.exe
if errorlevel 1 (
    echo [ERREUR] Echec de compilation de MimiMax.exe.
    pause
    exit /b 1
)
copy /Y MimiMax.exe Aether-Launcher.exe >nul
if errorlevel 1 (
    echo [ERREUR] Impossible de copier Aether-Launcher.exe.
    pause
    exit /b 1
)

if not exist "launcher\desktop\target\release\aether-desktop.exe" (
    echo [ERREUR] L'executable Tauri attendu est introuvable.
    pause
    exit /b 1
)

echo.
echo ========================================================
echo   SUCCES : MimiMax.exe et Aether-Launcher.exe sont prets !
echo   Ils ouvrent le lanceur natif Rust + Tauri.
echo ========================================================
endlocal

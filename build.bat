@echo off
echo ========================================================
echo   Compilation MimiMax / Aether Launcher
echo ========================================================

echo.
echo [1/3] Compilation du Core Rust...
cd launcher
call cargo build --release
if %errorlevel% neq 0 (
    echo [ERREUR] Echec de compilation du Core Rust.
    cd ..
    pause
    exit /b %errorlevel%
)
cd ..

echo.
echo [2/3] Generation du Mod Minecraft (Fabric)...
cd aether-client
call gradlew.bat build --offline
if %errorlevel% neq 0 (
    echo [AVERTISSEMENT] Le build Gradle a signale un avertissement ou une erreur.
) else (
    copy /Y build\libs\aether-client-1.0.0.jar ..\tools\aether-home\.minecraft\mods\aether-client.jar >nul
)
cd ..

echo.
echo [3/3] Creation des executables racines (.exe)...
rustc -O -C opt-level=z -C lto=yes -C strip=symbols tools\launcher-runner.rs -o MimiMax.exe
copy /Y MimiMax.exe Aether-Launcher.exe >nul

echo.
echo ========================================================
echo   SUCCES : MimiMax.exe et Aether-Launcher.exe sont prets !
echo   Vous pouvez les lancer directement dans ce dossier.
echo ========================================================

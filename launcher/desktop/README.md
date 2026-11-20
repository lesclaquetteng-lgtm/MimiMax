# Aether Launcher — native shell (Tauri 2)

The window of the launcher. It is a Rust application built on **Tauri 2**, which serves the interface
to the **WebView2** runtime that ships with Windows 10 and 11. There is no bundled browser and no
Node.js runtime: the shell is a single ~8.75 MB executable that carries the launcher core, the client
mod and the interface files, and the installer is 4.42 MB.

```
desktop/
├── src/main.rs         window, single-instance lock, command registration, cleanup on exit
├── src/commands.rs     one #[tauri::command] per interface call (39 commands)
├── src/core.rs         runs the core, streams its JSON events, embeds the binary and the mod, kills trees
├── src/state.rs        the processes the window can stop (installation, game)
├── build.rs            refuses to build before the core exists
├── tauri.conf.json     window, bundling, French NSIS, branding, licence
├── installer/          LICENSE-FR.txt, header.bmp, sidebar.bmp
└── icons/              application icons (including the .ico of the installer)
```

The interface lives in `../gui/src/ui/` and is served by WebView2. The `bridge.js` module
exposes `window.aether` through Tauri's `invoke`; account, profile, installation and game-process
operations remain inside the Rust core.

## Prerequisites

- **Rust** (stable) with the MSVC toolchain.
- **Tauri CLI**: `cargo install tauri-cli --version "^2"` (the commands below use `cargo tauri`).
- **NSIS**: `cargo tauri build` downloads and uses its own copy; nothing to install by hand.
- **WebView2**: present on Windows 10/11. The installer embeds the bootstrapper
  (`webviewInstallMode: embedBootstrapper`) for a machine that lacks it.

## Build

The core and the client mod are embedded with `include_bytes!`
(`../../target/release/aether-launcher.exe` and
`../../../aether-client/build/libs/aether-client-1.0.0.jar`). The build script refuses to compile
when either file is missing. Build the mod, then the core, then the shell:

```powershell
# From the repository root
cd aether-client
.\gradlew.bat --no-daemon build                    # 1. Fabric client JAR
cd ..\launcher
cargo build --release                                # 2. Rust core executable
cargo build --release --manifest-path desktop/Cargo.toml  # 3. native shell
```

Or run `build.bat` from the repository root to build the client, core, shell and portable launch
entries in that order.


## Installer

```powershell
cd launcher/desktop
cargo tauri build          # NSIS + the portable executable
```

Artefacts:

| File | Size | Role |
| --- | --- | --- |
| `target/release/aether-desktop.exe` | 8.75 MB | portable build, single file |
| `target/release/bundle/nsis/Aether Launcher_1.0.0_x64-setup.exe` | 4.42 MB | installer |

`dist/` keeps the published copies (`Aether-Launcher-1.0.0-portable.exe`,
`Aether-Launcher-Setup-1.0.0.exe`) with their SHA-256 in `dist/LISEZ-MOI.txt`.

What `tauri.conf.json` decides:

- `installMode: currentUser` — no administrator prompt; everything goes to
  `%LOCALAPPDATA%\Aether Launcher` (executable, `AETHER-LICENSE-FR.txt`, `uninstall.exe`).
- `languages: ["French"]` with `displayLanguageSelector: false` — the wizard is French only.
- `licenseFile: installer/LICENSE-FR.txt` — a real MUI licence page, with `J'accepte` /
  `Je n'accepte pas`. The file is UTF-8 without BOM; Tauri stages a BOM copy for NSIS, which needs
  one to show the accents correctly.
- `headerImage` / `sidebarImage` — `installer/header.bmp` and `installer/sidebar.bmp` (the branded
  pages in `docs/images/installer-*.png`).
- `startMenuFolder: "Aether"` — Start Menu shortcut under `Aether\`, plus the desktop shortcut.
- The licence file is also shipped as a resource (`AETHER-LICENSE-FR.txt`) next to the executable.

Measured on the shipped build:

| Action | Command | Result |
| --- | --- | --- |
| Silent install | `"Aether-Launcher-Setup-1.0.0.exe" /S` | exit 0, **1.6 s**, shortcuts and uninstall entry written |
| Five pages | `tools/installer-capture.ps1` | welcome 0.2 s, licence 1.3 s, folder 2.4 s, install 4.8 s, finish 5.5 s |
| Uninstall | `%LOCALAPPDATA%\Aether Launcher\uninstall.exe /S` | exit 0, **1.3 s**, folder gone in 1 s |

Two NSIS behaviours worth knowing when testing by hand:

- The chosen directory is remembered in `HKCU\Software\Aether\Aether Launcher` and the uninstaller
  does not delete that key. A stale value silently redirects the next install; remove the key when a
  test must start from a clean slate.
- The uninstaller stub returns immediately and continues from `%TEMP%\~nsu1.tmp\Un.exe`: poll until
  the installation directory disappears before reinstalling.

## Tests

```powershell
cargo test --release --manifest-path desktop/Cargo.toml   # 8 tests
cargo clippy --release --all-targets --manifest-path desktop/Cargo.toml
```

The unit tests cover the shape of the answers the commands build, the JSON documents of the core and
the timeouts of the network commands. The window itself is driven over the DevTools protocol.

### Driving the real window (CDP)

WebView2 exposes a Chrome DevTools endpoint when it is asked to:

```powershell
cd launcher/desktop
$env:AETHER_HOME = "C:\path\to\a\throwaway\home"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9343"
& .\target\release\aether-desktop.exe
```

Then, from the repository root:

```powershell
node tools/cdp.mjs 9343 "window.aether.accounts.checkName('Notch')"
node tools/ui-check.mjs 9343 tools/ui-shots     # every page, both sizes, overflow report
```

Caveats learned from the runs:

- **One instance only.** The single-instance plugin focuses the open window and exits 0 with no CDP
  port when a previous shell is still running. Kill it before starting the next one.
- **A throwaway home.** The e2e checks create and delete accounts and mods; `tools/e2e/guard.mjs`
  refuses any `AETHER_HOME` that is not under `tools/`.
- **The interface is the shipped one.** `ui-check.mjs` reads `docs/images`-quality data from the core,
  so an empty home shows empty pages. Seed `AETHER_HOME` (see `launcher/README.md`) or copy
  `tools/aether-home`.

## Installer page capture

`tools/installer-capture.ps1` runs the installer once, captures each page to
`docs/images/installer-<page>.png` and stops: it recognises the last page through the `Fermer`
button, then closes the wizard by ending its process, so the checked "Lancer Aether Launcher" box
never starts a second instance. `tools/installer-watch.ps1` prints the same pages with their timing
and the controls they contain, which is how a slow installer is told apart from a slow script.

## Licence

MIT, like the rest of the project. Minecraft is a trademark of Mojang Studios; this project is not
affiliated with Mojang or Microsoft.

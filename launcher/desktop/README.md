# CUBE Launcher — native shell (Tauri 2)

The window of the launcher. It is a Rust application built on **Tauri 2**, which serves the interface
to the **WebView2** runtime that ships with Windows 10 and 11. There is no bundled browser and no
Node.js runtime: the shell is a single ~8.90 MB executable that carries the launcher core, the client
mod and the interface files, and the installer is 4.53 MB.

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

The interface itself lives in `../gui/src/ui/` and is shared, unchanged, with the old Electron
build: `bridge.js` exposes `window.aether` over Tauri's `invoke`, so `app.js` never learned that the
shell changed.

## Prerequisites

- **Rust** (stable) with the MSVC toolchain.
- **Tauri CLI**: `cargo install tauri-cli --version "^2"` (the commands below use `cargo tauri`).
- **NSIS**: `cargo tauri build` downloads and uses its own copy; nothing to install by hand.
- **WebView2**: present on Windows 10/11. The installer embeds the bootstrapper
  (`webviewInstallMode: embedBootstrapper`) for a machine that lacks it.

## Build

The core and the client mod are embedded with `include_bytes!`
(`../../target/release/aether-launcher.exe` and
`../../../aether-client/build/libs/aether-client-1.0.0.jar`), and `build.rs` panics when either file
is missing. Build both first, always:

```powershell
cd launcher
cargo build --release                                  # 1. core  -> target/release/aether-launcher.exe
cargo build --release --manifest-path desktop/Cargo.toml  # 2. shell -> desktop/target/release/aether-desktop.exe
```

or, from `launcher/`:

```powershell
npm run core     # cargo build --release
npm run shell    # cargo build --release --manifest-path desktop/Cargo.toml
npm start        # runs the shell from its build directory
```

The shell finds the core in this order: `AETHER_LAUNCHER_BIN`, next to the shell, the build
directory during development, the bundled resources, then the embedded copy. At start it writes the
embedded core to `%LOCALAPPDATA%\dev.aether.launcher\bin\aether-launcher.exe`, compares the bytes on
the next start, and only rewrites the file when it differs.

The mod follows the same path: it is written to
`%LOCALAPPDATA%\dev.aether.launcher\mods\aether-client.jar` and handed to the core as
`AETHER_MOD_JAR`. Without that archive an installed launcher cannot run "Install / repair": the core
only looks for the jar next to itself and in the folders of the project, and a machine that received
the installer has neither. `tools/checks/clean-install-check.ps1` proves both halves, from an empty folder
and an empty home: the bare core reports `the Aether client jar was not found`, while the shell
extracts the archive and the installation starts.

> A running `aether-desktop.exe` (or the core it started) locks the target executable: close the
> window before rebuilding, or the link step fails with "access denied".

## Installer

```powershell
cd launcher/desktop
cargo tauri build          # NSIS + the portable executable
```

Artefacts:

| File | Size | Role |
| --- | --- | --- |
| `target/release/aether-desktop.exe` | 8.90 MB | portable build, single file |
| `target/release/bundle/nsis/CUBE Launcher_1.0.0_x64-setup.exe` | 4.53 MB | installer |

`dist/` keeps the published copies (`CUBE-Launcher-1.0.0-portable.exe`,
`CUBE-Launcher-Setup-1.0.0.exe`) with their SHA-256 in `dist/LISEZ-MOI.txt`.

What `tauri.conf.json` decides:

- `installMode: currentUser` — no administrator prompt; everything goes to
  `%LOCALAPPDATA%\CUBE Launcher` (executable, `AETHER-LICENSE-FR.txt`, `uninstall.exe`).
- `languages: ["French"]` with `displayLanguageSelector: false` — the wizard is French only.
- `licenseFile: installer/LICENSE-FR.txt` — the licence read by the custom page. It is stored in
  Windows-1252: `FileRead` decodes with the ANSI code page, so UTF-8 accents would arrive mangled.
  Tauri stages a BOM copy for NSIS; the page strips those three bytes at run time.
- `sidebarImage` — `installer/sidebar.bmp`, the 760x440 CUBE panel drawn behind the page
  (`tools/make-brand.ps1`).
- `template: installer/installer.nsi` — replaces the stock MUI wizard with one custom page (below).
- `startMenuFolder: "CUBE"` — Start Menu shortcut under `CUBE\`, plus the desktop shortcut.
- The licence file is also shipped as a resource (`AETHER-LICENSE-FR.txt`) next to the executable.

### The custom page

`installer/installer.nsi` draws a frameless 760x440 window: the wizard toolbar is stripped from
`$HWNDPARENT`, the art becomes a bitmap child pushed to the bottom of the z-order, and one page
carries five states instead of the stock five pages.

| State | Content |
| --- | --- |
| 0 | welcome: features, version, `ANNULER` / `INSTALLER` |
| 1 | licence sheet (`installer/LICENSE-FR.txt`), a `J'accepte...` toggle gating `CONTINUER` |
| 2 | install folder, `PARCOURIR`, two toggles (desktop shortcut, run after install) |
| 3 | progress: big percentage, status line, brand-green bar |
| 4 | done, `TERMINER`, opens the launcher when the box was left checked |

Every button is a `SS_NOTIFY` static drawn in the CUBE palette (`SetCtlColors` takes `0xRRGGBB`
here): native push buttons ignore those colours and looked foreign. Screenshots of the states live
in `docs/images/installer-{1..5}-*.png`.

State 3 is the only one that lasts. `SetProgress` pushes the bar value, the percentage and the
status line onto screen (an `UpdateWindow` on each) and then holds `PROGRESS_HOLD_MS` (130 ms), so
the stages reported by `InstallCore` climb 12 -> 38 -> 66 -> 80 -> 94 -> 97 -> 100 in visible steps
over roughly 1.7 s instead of flashing to the done page. NSIS blocks its own message loop while a
page callback runs, so the explicit repaint before each `Sleep` is what makes the motion visible:
without it nothing is drawn until the page changes, and a forced repaint is expensive enough
(~200 ms here) that a dozen stages is a sensible ceiling.

The install folder is not a Windows `EDIT` box either: a 1 px `CLR_FIELD` frame drawn from four
hairline statics surrounds a borderless `EDIT` (`0x50010080`) painted `CLR_BG_INPUT`. `SetCtlColors`
does work on an EDIT here, but only once the theme is switched off and `WS_BORDER` /
`WS_EX_CLIENTEDGE` are gone; the stock `${NSD_CreateText}` keeps its white box and light border.

Every state change ends with `PulseAccent`: a 2 px brand tick under the kicker growing 12 -> 44 px
in three 45 ms frames. It is the only motion between screens, and it repaints a 44x2 static rather
than the page, so it costs almost nothing.

Silent mode, `/P`, `/UPDATE`, the shortcuts, the registry entry and `uninstall.exe` keep working.
This template has no `instfiles` page, so sections never run on their own; the page calls
`InstallCore` / `InstallWebView2` in every mode.

- `/S` never creates the page, so `SetProgress` sees `$hProg = 0` and skips both the repaint and the
  hold: a silent install stays around 2 s.
- `/P` and `/UPDATE` open straight on the progress sheet (`$Step 3`), then a one-shot
  `${NSD_CreateTimer}` runs `RunUnattended`. The timer is needed because the page only becomes
  visible after `PageBrand` returns; it is killed on entry so the install runs once.
- `Quit` does nothing while nsDialogs runs the page loop: unattended runs end by posting `WM_COMMAND`
  with `IDOK` to `$HWNDPARENT`, which leaves the only page and exits with code 0. `/P` shows the done
  sheet for 1.2 s first, `/UPDATE` closes as soon as the files are in place.

Two traps that cost a build each:

- `nsDialogs::CreateControl` takes `class style exstyle x y w h text`. Dropping the `exstyle`
  argument shifts every coordinate and silently yields zero-height controls.
- Controls are created at the top of the z-order and the page repaints its background over its
  children. The page needs `WS_CLIPCHILDREN`, the art and the drawn buttons need
  `WS_CLIPSIBLINGS`, and a revealed control (`ShowWindow` + `InvalidateRect` + `UpdateWindow`) must
  be repainted explicitly.

Measured on the shipped build:

| Action | Command | Result |
| --- | --- | --- |
| Silent install | `"CUBE-Launcher-Setup-1.0.0.exe" /S` | exit 0, files, shortcuts and uninstall entry written |
| Silent uninstall | `%LOCALAPPDATA%\CUBE Launcher\uninstall.exe /S` | exit 0, folder, shortcuts and uninstall entry gone |

Two NSIS behaviours worth knowing when testing by hand:

- The chosen directory is remembered in `HKCU\Software\Aether\CUBE Launcher` and the uninstaller
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
node tools/checks/cdp.mjs 9343 "window.aether.accounts.checkName('Notch')"
node tools/checks/ui-check.mjs 9343 tools/ui-shots     # every page, both sizes, overflow report
```

Caveats learned from the runs:

- **One instance only.** The single-instance plugin focuses the open window and exits 0 with no CDP
  port when a previous shell is still running. Kill it before starting the next one.
- **A throwaway home.** The e2e checks create and delete accounts and mods; `tools/e2e/guard.mjs`
  refuses any `AETHER_HOME` that is not under `tools/`.
- **The interface is the shipped one.** `ui-check.mjs` reads `docs/images`-quality data from the core,
  so an empty home shows empty pages. Seed `AETHER_HOME` (see `launcher/README.md`) or copy
  `tools/aether-home`.

## Licence

MIT, like the rest of the project. Minecraft is a trademark of Mojang Studios; this project is not
affiliated with Mojang or Microsoft.

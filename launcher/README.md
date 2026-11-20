# Aether Launcher

Launcher for **Aether Client**. It installs Minecraft with Fabric and the Aether mod, then starts the game with an **offline** or **Microsoft** account.

## Architecture

The primary desktop application is native **Rust + Tauri 2**:

- **Rust core** (`src/`, `Cargo.toml`): account storage, named profiles, Java detection, Minecraft/Fabric metadata, installs, launch arguments, game process management, mods, servers and performance tools.
- **Tauri desktop shell** (`desktop/`): native window, single-instance guard, safe process lifecycle, Rust commands and packaging. The core is embedded in the Windows desktop build.
- **Shared interface** (`gui/src/ui/`): HTML/CSS/JavaScript used by Tauri. The same interface can still be run through the legacy Electron shell while the migration is validated.

The UI never launches Java directly. It calls a Rust command; the Rust core validates the operation, installs or launches Minecraft and streams progress back to the interface.

## Requirements

- Rust stable + Cargo.
- Windows: MSVC C++ Build Tools and WebView2 Runtime (the installer uses the Tauri WebView2 bootstrapper).
- Java 21 for Minecraft 1.21.11. The launcher detects compatible Java installations and also accepts a custom executable path.
- Node.js + npm only when using the convenience scripts, the legacy Electron fallback or the Node-based UI tests.

## Run from source (Windows)

From this directory:

```powershell
# Build the Rust game-management core, then start the native Tauri shell.
npm start

# Equivalent explicit commands:
cargo build --release
cargo run --manifest-path desktop/Cargo.toml
```

The shell build embeds `target/release/aether-launcher.exe`. Always rebuild the core before rebuilding the desktop shell.

## Build a distributable installer

Install the Tauri CLI once, then build the release installer:

```powershell
cargo install tauri-cli --version "^2"
npm run build:desktop
npm run dist:desktop
```

The Tauri configuration currently targets a Windows NSIS installer. The release executable is produced under `desktop/target/release/`.

## Legacy Electron fallback

Electron remains available for comparison and emergency fallback; it is not the default desktop shell:

```powershell
npm install
npm run start:electron
```

The old Electron Builder commands are retained as `npm run pack:electron` and `npm run dist:electron`. They are not the recommended release path.

## Rust CLI

The core prints JSON documents, which are also the contract used by the desktop shell.

```powershell
cargo run --release -- help
# or:
.\target\release\aether-launcher.exe help
```

Available commands:

| Command | Description |
| --- | --- |
| `info` | Paths, settings, active profile and Java runtime information |
| `settings [--key <key> --value <value>]...` | Read or update settings |
| `accounts` | List accounts and the selected account |
| `account add-offline <username>` | Create/select an offline account |
| `account microsoft` | Start Microsoft device-code sign-in |
| `versions [--limit N]` | List Minecraft releases |
| `loaders [--version X]` | List Fabric loader versions |
| `profiles [list|add|update|assign|remove]` | Manage named game profiles |
| `install [--version X] [--loader Y]` | Install/repair Minecraft, libraries, assets and Fabric |
| `launch [--version X] [--account ID] [--server ADDRESS]` | Start Minecraft and stream game output |
| `mods [list|toggle|search|install]` | Manage local mod jars and discover/install Fabric mods from Modrinth |
| `servers [list|add|remove|ping|probe]` | Saved servers and offline-mode checks |
| `hosts [list|create|start|stop]` | Local Fabric server hosting |
| `performance [list|install|tune|jvm]` | Verified optimisation pack and reversible presets |
| `logs [read]` | Installation and game-session logs |

## Modrinth mod library

The Mods page has two views: **Installed** for local JARs and **Discover** for compatible Fabric mods.
Search results are filtered for the Minecraft version of the active profile. Installing a project
resolves its required dependencies, downloads the published JARs from the Modrinth CDN and verifies
their SHA-1 hashes before adding them to the game.

Aether stores its managed project/version/file/checksum records in
`<launcher-data>/modrinth-mods.json`. When updating a project it only removes older files whose
hash still matches that manifest; manually placed JARs are left untouched. Mod changes are blocked
while Minecraft is starting or running.

CLI examples:

```powershell
# Search compatible Fabric mods for the active profile (or pass --version explicitly)
aether-launcher mods search sodium
aether-launcher mods search minimap --version 1.21.11

# Install a Modrinth project by its project ID; required dependencies are included
aether-launcher mods install AANobbMI
```

## Data and environment variables

| Variable | Purpose |
| --- | --- |
| `AETHER_HOME` | Override the launcher data root |
| `AETHER_SHARED_DIR` | Use a specific Minecraft game directory |
| `AETHER_ANCHOR` | Locate game assets/mod jars delivered next to a portable build |
| `AETHER_MOD_JAR` | Explicit Aether client mod jar path |
| `AETHER_LAUNCHER_BIN` | Override the Rust core executable (useful for development/tests) |

Default paths:

- Settings, accounts and logs: `<home>/Aether/aether`
- Minecraft game data: `<home>/Aether/.minecraft`

Offline accounts use the deterministic Minecraft-compatible `OfflinePlayer:<name>` UUID. They work in singleplayer and on servers configured with `online-mode=false`; they do not bypass premium authentication on online-mode servers.

## Project layout

```text
launcher/
├── src/                  Rust game-management core and CLI
├── desktop/              Tauri 2 native desktop shell
│   ├── src/              Rust commands, process runner and lifecycle
│   └── tauri.conf.json   Windows installer and WebView configuration
├── gui/src/ui/           Shared launcher interface
├── gui/src/              Legacy Electron fallback and preload
├── tests/                Rust and Node regression tests
├── Cargo.toml
└── package.json          Development and build shortcuts
```

## Tests

```powershell
# Rust core
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings

# Native Tauri command and lifecycle tests (build the core first because it is embedded)
cargo build --release
cargo test --manifest-path desktop/Cargo.toml

# Shared renderer and Electron-fallback checks
node --check gui/src/ui/app.js
node --check gui/src/ui/bridge.js
node --test tests/ui-contract.test.cjs
npm run test:security
```

The Windows CI also builds the native shell and exercises account/profile/keyboard flows through the desktop runtime before a release is considered ready.

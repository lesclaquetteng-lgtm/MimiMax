# Aether Launcher

Launcher for **Aether Client**. It installs Minecraft with the Fabric loader and the Aether mod, and
starts the game with an **offline account** or a **Microsoft account**.

The launcher is split into two parts:
- **Rust core** (`src/`, `Cargo.toml`): accounts, settings, version/Fabric metadata, installation,
  classpath/argument building, game launch, and a JSON-oriented CLI.
- **Electron GUI** (`gui/`): desktop interface that spawns the Rust binary and streams progress/output.

## Prerequisites

- **Rust toolchain** (Cargo) to build and test the core.
- **Node.js + npm** to run/build the Electron UI.
- **Java runtime** available for launching Minecraft (the launcher detects/configures the Java path).

## Build & run

### 1) Build the launcher core
```powershell
cargo build --release
# or: npm run core
```

The GUI looks for:
- `target/release/aether-launcher(.exe)`
- `target/debug/aether-launcher(.exe)`
- or the binary specified by `AETHER_LAUNCHER_BIN`

### 2) Start the GUI
```powershell
npm install
npm start
```

> If the core binary is missing, the GUI reports that it must be built first.

## CLI (Rust core)

The CLI prints JSON messages (useful for scripting and for the GUI bridge).

```powershell
cargo run --release -- help
# or: .\target\release\aether-launcher.exe help
```

Available commands:

| Command | Description |
| --- | --- |
| `info` | Print paths and current settings |
| `settings [--key <camelCaseKey> --value <value>]...` | Read settings, or update key/value pairs and print the result |
| `accounts` | List stored accounts and the selected account |
| `account add-offline <username>` | Create/select an offline account |
| `account remove <id>` | Remove an account |
| `account select <id>` | Select the active account |
| `account microsoft` | Start Microsoft device-code sign-in (emits device code, then account/result messages) |
| `versions [--limit N]` | List released Minecraft versions |
| `loaders [--version X]` | List Fabric loader versions for a game version |
| `install [--version X] [--loader Y] [--fabric-api Z]` | Install game/libraries/assets/Fabric API/mod and emit progress events |
| `launch [--version X] [--account ID]` | Launch Minecraft and stream game output/exit status |
| `command [--version X] [--account ID]` | Print the resolved Java program, arguments and classpath |

### Notes
- Default profile values come from settings (e.g. `gameVersion`, `loaderVersion`, `fabricApiVersion`).
- Installation requires the Aether client mod jar to be discoverable (see `AETHER_MOD_JAR` / `find_client_jar` logic in `src/settings.rs`).
- Some long-running commands emit line-delimited JSON events (e.g. `type: "progress"`, `type: "output"`, `type: "deviceCode"`).

## Data & environment variables

| Variable | Purpose |
| --- | --- |
| `AETHER_HOME` | Override the launcher root directory |
| `AETHER_SHARED_DIR` | Use a specific game directory (e.g. an existing `.minecraft`) |
| `AETHER_ANCHOR` | Directory used to locate an installation sitting next to the executable (set by the shell) |
| `AETHER_MOD_JAR` | Explicit path to `aether-client-*.jar` for installation |
| `AETHER_LAUNCHER_BIN` | (GUI) Explicit path to the launcher core binary |

Resolved paths (defaults, unless overridden):

| What | Path |
| --- | --- |
| Launcher data (settings, accounts, logs) | `<home>/Aether/aether` |
| Game directory (assets, libraries, versions, mods) | `<home>/Aether/.minecraft` |

`<home>` is derived from the current user profile/home directory.

## Offline mode

For offline accounts, the launcher computes the UUID exactly like Minecraft:

`UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`

The game is started with the offline username, that deterministic UUID, and `accessToken` set to `0`.
The Aether mod detects offline sessions in-game and adjusts session-specific behavior accordingly.

Implementation: `src/offline.rs`, `src/accounts.rs`, `src/game.rs`.

## Project layout

```text
launcher/
├── src/                  Rust core (CLI, install, launch, metadata, accounts, settings)
├── gui/
│   └── src/
│       ├── main.js       Electron main process (spawns core, IPC handlers)
│       ├── preload.js    Renderer bridge (window.aether API)
│       └── ui/           Renderer UI (HTML/CSS/JS)
├── tests/                Rust integration/unit tests
├── Cargo.toml
└── package.json          Electron scripts/build config
```

## Tests & checks

```powershell
cargo test
# optional
cargo clippy --all-targets
```

GUI packaging/build scripts are defined in `package.json` (`pack`, `dist`) and require Electron Builder
dependencies/configuration appropriate for your environment.

# Aether Launcher

Launcher for **Aether Client**. It installs Minecraft with the Fabric loader and the Aether mod, and
starts the game with an **offline account** or a **Microsoft account**.

The launcher is split into two parts:

- **Rust core** (`src/`, `Cargo.toml`): accounts, settings, version/Fabric metadata, installation,
  classpath/argument building, game launch, and a JSON-oriented CLI.
- **Native shell** (`desktop/`, Tauri 2 + WebView2): the window, the single-instance lock, the
  process runner, the packaging — and the interface it serves from `gui/src/ui/`.

The interface is HTML/CSS/JavaScript served by the system WebView2 runtime.
`gui/src/ui/bridge.js` exposes `window.aether` through Tauri IPC; the renderer does not own
accounts, settings, installation or game processes. Those operations remain in Rust.

## Prerequisites

- **Rust toolchain** (Cargo) to build and test the core and the shell.
- **WebView2** (shipped with Windows 10/11; the installer embeds the bootstrapper otherwise).
- **Java runtime** available for launching Minecraft (the launcher detects/configures the Java path).
- **Node.js** only for the end-to-end scripts in `../tools/e2e/`.

## Build & run

The native shell embeds **both** the Rust core executable and the Fabric client JAR. Build these
prerequisites before compiling the shell:

```powershell
# From the repository root: build the Fabric mod embedded by Tauri.
cd aether-client
.\\gradlew.bat --no-daemon build

# Build the game-management core and then the native desktop shell.
cd ..\\launcher
cargo build --release
cargo build --release --manifest-path desktop/Cargo.toml
```

The build script checks both embedded files and stops with a specific message when either is missing.
Rebuild the shell after changing the core, the mod JAR or any shared interface asset.

To run it from `launcher/`:

```powershell
npm run core      # cargo build --release
npm run shell     # cargo build --release --manifest-path desktop/Cargo.toml
npm start         # runs the native Tauri shell
npm run test:shell
```

For a clean portable build with both root entry points, run `build.bat` from the repository root.
It builds the Fabric mod, the Rust core, the Tauri shell and finally the small Rust executable that
opens the shell.

## CLI (Rust core)

The CLI prints JSON messages (the interface is a client of these commands).

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
| `account check <username>` | Ask Mojang whether a name belongs to a Mojang account (`premium`), before a cracked server can refuse it |
| `account microsoft` | Start Microsoft device-code sign-in (emits device code, then account/result messages) |
| `versions [--limit N]` | List released Minecraft versions |
| `loaders [--version X]` | List Fabric loader versions for a game version |
| `install [--version X] [--loader Y] [--fabric-api Z]` | Install game/libraries/assets/Fabric API/mod and emit progress events |
| `launch [--version X] [--account ID]` | Launch Minecraft and stream game output/exit status |
| `stop` | Stop the recorded game, even when the launcher that started it is gone |
| `mods search <query> --game-version X` | Search Modrinth for Fabric mods compatible with a Minecraft version |
| `mods install <project-id> --game-version X` | Install a mod and its required dependencies, checking the official CDN and SHA-1 |
| `command [--version X] [--account ID]` | Print the resolved Java program, arguments and classpath |

### The record of the game

A launch writes the identifier of the Java process to `<home>/aether/game.pid` and removes it when
the game exits. The shell kills the child it started when the window closes; the record covers the
case where the shell itself was killed, or when the game was started from the command line: `stop`
reads it and kills the tree instead of leaving an orphan that the next session cannot name.

### Notes
- Default profile values come from settings (e.g. `gameVersion`, `loaderVersion`, `fabricApiVersion`).
- Installation requires the Aether client mod jar to be discoverable (see `AETHER_MOD_JAR` / `find_client_jar` logic in `src/settings.rs`).
- Some long-running commands emit line-delimited JSON events (e.g. `type: "progress"`, `type: "output"`, `type: "deviceCode"`).

Example:

```powershell
.\\target\\release\\aether-launcher.exe mods search Sodium --game-version 1.21.11
.\\target\\release\\aether-launcher.exe mods install <project-id> --game-version 1.21.11
```

Modrinth results are filtered to Fabric projects for the selected Minecraft version. Installation is
performed by the Rust core, installs dependencies marked `required`, verifies the official CDN and
SHA-1, and never overwrites a different JAR. Mods are third-party executable code; review projects
before installing them.

## Data & environment variables

| Variable | Purpose |
| --- | --- |
| `AETHER_HOME` | Override the launcher root directory |
| `AETHER_SHARED_DIR` | Use a specific game directory (e.g. an existing `.minecraft`) |
| `AETHER_ANCHOR` | Directory used to locate an installation sitting next to the executable (set by the shell) |
| `AETHER_MOD_JAR` | Explicit path to `aether-client-*.jar` for installation |
| `AETHER_LAUNCHER_BIN` | (shell) Explicit path to the launcher core binary |

Resolved paths (defaults, unless overridden):

| What | Path |
| --- | --- |
| Launcher data (settings, accounts, logs) | `<home>/Aether/aether` |
| Record of the running game | `<home>/Aether/aether/game.pid` |
| Game directory (assets, libraries, versions, mods) | `<home>/Aether/.minecraft` |

`<home>` is derived from the current user profile/home directory.

## Offline mode

For offline accounts, the launcher computes the UUID exactly like Minecraft:

`UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`

The game is started with the offline username, that deterministic UUID, and `accessToken` set to `0`.
The Aether mod detects offline sessions in-game and adjusts session-specific behavior accordingly.

Cracked servers accept that session, but nearly all of them refuse a name that a Mojang account
already owns: the game then only shows `Unauthorized: You cannot join the server`. `account check`
answers the question before the launch (`200` = premium, `204`/`404` = free), and the interface shows
the same warning under the name field and in a dialog when a public server is joined. A failed lookup
never blocks a launch: the game stays the judge.

Implementation: `src/offline.rs`, `src/accounts.rs`, `src/game.rs`.

## Project layout

```text
launcher/
├── src/                  Rust core (CLI, install, launch, metadata, accounts, settings)
├── desktop/              Native shell (Tauri 2), see desktop/README.md
│   ├── src/main.rs       window, single-instance lock, command registration
│   ├── src/commands.rs   one command per interface call (same names as the old IPC)
│   ├── src/core.rs       runs the core, streams its JSON events, embeds the binary and the mod
│   ├── src/state.rs      the processes the window can stop (install, game)
│   ├── installer/        French licence page and installer branding
│   └── tauri.conf.json   window, CSP, NSIS (French) packaging
├── gui/src/ui/           the interface: HTML/CSS/JS + bridge.js (window.aether over Tauri IPC)
├── tests/                Rust integration/unit tests
├── Cargo.toml
└── package.json          npm shortcuts (core, shell, start, e2e)
```

## Tests & checks

```powershell
cargo test                                               # core
cargo test --manifest-path desktop/Cargo.toml            # shell
cargo clippy --all-targets                               # core
cargo clippy --all-targets --manifest-path desktop/Cargo.toml
```

The end-to-end scripts live in `../tools/e2e/` and drive the real window over the DevTools protocol
of WebView2 (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"`); they use the
same `window.aether` bridge as a player. See `../docs/VERIFICATION.md` §4.

Current counts: **134 unit + 6 download + 20 integration + 3 SHA-1 = 163** tests in the core (all
green), **8** in the shell, and **87** (16 suites) in the Fabric mod. `desktop/README.md` covers the
Tauri build, the installer and the constraint that the core must be built first.

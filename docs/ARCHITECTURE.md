# Architecture

This document describes how the current implementation is structured and the main design decisions.

## Overview

```text
┌────────────────────────────────────┐        ┌──────────────────────────────────┐
│          Aether Launcher           │        │           Aether Client          │
│       (Rust core + Tauri 2)       │        │            (Fabric mod)          │
│                                    │        │                                  │
│  Rust core (src/*.rs, CLI/JSON):   │        │  AetherClient / Aether           │
│   - accounts, settings, paths      │        │  ModuleManager / HudManager      │
│   - minecraft metadata & rules     │  args  │  Click GUI / Config / HUD editor │
│   - installer (download/verify)    │───────►│  Mixins + session/config logic   │
│   - game (classpath & launch)      │classpath│                                  │
│                                    │        │                                  │
│  Tauri desktop (desktop/src/*):   │        │                                  │
│   - native Rust window + commands │        │                                  │
│   - embeds core, streams events    │        │                                  │
└────────────────────────────────────┘        └──────────────────────────────────┘
```

The launcher prepares and starts a Minecraft/Fabric process; the mod runs inside that process and
provides the client-side features.

## Launcher architecture

### Rust core (`launcher/src`)

The core contains all business logic and exposes a command-line interface (`main.rs`) that emits
JSON lines/events for the GUI.

| Module | Responsibility |
| --- | --- |
| `main.rs` | CLI parsing/dispatch, JSON output, streaming wrappers for install/launch/auth flows. |
| `lib.rs` | High-level `Launcher` API tying paths, settings, accounts, install and launch together. |
| `paths.rs` | Resolution/creation of launcher and game directories (`AETHER_HOME`, `AETHER_SHARED_DIR`). |
| `settings.rs` | Persistent settings, Java detection, mod jar discovery (`AETHER_MOD_JAR`). |
| `accounts.rs` | Account store, offline accounts, Microsoft device-code login flow and session mapping. |
| `offline.rs` | Offline username validation and deterministic Minecraft-compatible offline UUID generation. |
| `minecraft.rs` | Mojang/Fabric metadata retrieval, rule evaluation, argument templating, library resolution/merging. |
| `http.rs` | HTTP helpers, JSON/text/bytes requests, file download with checksum verification, SHA-1 utilities. |
| `installer.rs` | Version installation pipeline (client, libraries, assets, Fabric API, natives, mod copy) with progress reporting. |
| `game.rs` | Classpath construction, JVM/game argument construction, process launch and output forwarding. |
| `servers.rs` | Saved server list, Server List Ping, login probe that tells an offline-accepting server from a premium-only one. |
| `hosting.rs` | Local Fabric server: shared installer, `server.properties`, RCON shutdown, process liveness. |
| `performance.rs` | Optimisation pack and reversible JVM/options presets with SHA-1 verification. |
| `modrinth.rs` | Fabric mod discovery, required dependency resolution, checksum-verified downloads and managed mod manifest. |

Key properties:
- **Single source of truth for launch/install logic** in Rust; the GUI does not duplicate it.
- **JSON CLI contract** enables scripting and decouples UI from core implementation details.
- **Checksum-verified downloads** (SHA-1 where provided/expected) to avoid corrupted/cached-bad files.

### Native Tauri shell and shared UI (`launcher/desktop`, `launcher/gui/src/ui`)

| File | Responsibility |
| --- | --- |
| `desktop/src/main.rs` | Tauri 2 entry point, native window, single-instance guard, Rust command registry and process shutdown. |
| `desktop/src/commands.rs` | Validates desktop operations and maps UI commands to the Rust CLI core. |
| `desktop/src/core.rs` / `state.rs` | Embedded core runner, streaming events, process lifecycle and cross-operation locks. |
| `gui/src/ui/*` | Shared HTML/CSS/JavaScript renderer with `bridge.js` for typed Tauri commands and events. |

The shared UI invokes typed Tauri commands (`info`, `settings`, `accounts`, `profiles`, `mods`,
`servers`, `install`, `launch`, etc.) and renders structured progress/output events emitted by Rust.
Electron remains an optional fallback shell only; it is not the default build or run path.

The native shell holds exclusive operation guards across asynchronous commands. A game launch cannot
race with a Minecraft install, Modrinth JAR install, mod toggle or `options.txt` tuning; conversely,
file-changing operations are rejected while Minecraft is running. Guards release automatically when
a command completes or exits early.

### Modrinth discovery (`launcher/src/modrinth.rs`)

The Discover tab queries the official Modrinth API with facets for the active Minecraft version and
Fabric loader. Installation resolves the compatible project version and required dependency chain,
then downloads JARs through the shared HTTP client. URLs must use HTTPS and the exact Modrinth CDN
host; filenames are constrained to safe `.jar` names and the API-provided SHA-1 is checked before a
file is accepted. The managed manifest tracks project/version/file/hash ownership so updates remove
only intact files previously installed by Aether, never a player's manually placed mods.

Search and install are Rust core commands. The Tauri shell forwards typed `mods:progress` events to
the same renderer that displays the installed-mod list.

## Launcher data flow

1. **Open/load**: resolve paths → load settings/accounts → detect/configure Java if needed.
2. **Install**: resolve metadata (vanilla + Fabric profile) → download/verify artifacts → place mod/API jars → prepare natives/assets/indexes.
3. **Launch**: build merged version/classpath/arguments for selected account and profile → spawn Java process → stream logs/exit code.
4. **Servers**: saved list → ping (`SLP`) → login probe (`Login Start` and the first answer of the server) → join with `--quickPlayMultiplayer`.
5. **Hosting**: Fabric installer (downloaded once, shared) → `server.properties` with `online-mode=false` and RCON → start → wait for the ping → stop through RCON, then kill the tree if the server hangs.
6. **Performance**: resolve each slug on Modrinth for the game version → download and verify the SHA-1 → write the manifest → write `options.txt` (backup first) → pick the JVM flags for the next launch.

### Offline-account probe

The probe never guesses from the MOTD or from the name of the server. It plays the beginning of a real
login with the name of the player:

| First packet of the server | Meaning |
| --- | --- |
| `0x01 Encryption Request` | `online-mode=true`; the server verifies the session with Mojang, a Microsoft account is required. |
| `0x02 Login Success`, `0x03 Set Compression` | The server accepted the name: `online-mode=false` (or a proxy that authenticates itself). |
| `0x00 Disconnect` | The server refused, and its own message is reported. |

The protocol version used is the one the server reports in its ping, so the probe stays valid when
the game version changes.

## Offline session design

Offline identity must match Minecraft/server expectations for `online-mode=false` servers.

- UUID derivation: MD5 of `OfflinePlayer:<username>` (version 3 / variant-adjusted), identical to
  Java's `UUID.nameUUIDFromBytes(...)`.
- Launcher behavior: pass `--username`, deterministic `--uuid`, and `--accessToken 0`.
- Client behavior: detect offline sessions and avoid online-only user services/flows where relevant.

This keeps player identity stable across launches for the same username while avoiding Mojang auth.

## Client (mod) architecture (`aether-client/src/main/java/dev/aether/client`)

The mod is a Fabric client mod (Mojang mappings, Java 21 target) organized around a small core and
feature registries.

| Area | Responsibility |
| --- | --- |
| `AetherClient` / `Aether` | Entrypoint/bootstrap, singleton/state access, lifecycle wiring. |
| `config/` | Persistent configuration load/save and module/HUD state serialization. |
| `module/` | Module framework, categories, registry, lifecycle callbacks. |
| `module/modules/` | Concrete modules (HUD/render/combat/movement/misc). |
| `setting/` | Typed module settings and UI-editable values. |
| `hud/` | HUD element management, layout/position persistence, rendering integration. |
| `gui/` | Click GUI, config screen, HUD editor and rendering helpers. |
| `util/` | Shared utilities (render/color/animation/session/click tracking, etc.). |
| `mixin/` | Targeted game patches where Fabric hooks/events are insufficient. |

### Mixins (principle)

Mixins are kept minimal and used only when no suitable API hook exists (e.g. UI replacement,
input/click capture, HUD/chat behavior adjustments). Tests in `src/test/java` validate mixin/config
consistency against the target game classes where applicable.

## Configuration and persistence

- **Launcher**: settings and accounts stored under the launcher data directory (e.g. `<home>/Aether/aether`).
- **Client**: module/HUD/config state stored in `config/aether/config.json` inside the game/config context.
- **Game files**: assets, libraries, versions and mods are installed in the configured game directory.
- **Servers**: the saved list lives in `aether/servers.json`, each hosted server owns a directory under
  `<home>/servers/<name>` with its own `server.properties`, world and `console.log`.
- **Performance**: the record of the installed mods is `aether/performance.json`; the first tuning
  writes `options.txt.aether-backup` next to the game options.

The client configuration tolerates a UTF-8 byte order mark: an editor such as Notepad adds one, and
Gson refuses a document that starts with it — without that tolerance the whole configuration would
silently fall back to the defaults.

## Build/test components

- **Launcher core**: Cargo project (`launcher/Cargo.toml`), CLI and Rust tests under `launcher/tests/` plus module tests.
- **Native desktop**: Tauri 2 Rust project (`launcher/desktop/Cargo.toml`) with command/lifecycle tests; Windows CI builds the shell.
- **Shared UI**: `launcher/gui/src/ui`, tested with Node renderer contracts and the real WebView2/Tauri end-to-end flows.
- **Fallback shell**: optional Electron wrapper, covered by syntax/security tests; not the default production launcher.
- **Client**: Gradle + Fabric Loom build (`aether-client/build.gradle`), JUnit tests under `src/test/java`.

# Architecture

This document describes how the current implementation is structured and the main design decisions.

## Overview

```text
┌────────────────────────────────────┐        ┌──────────────────────────────────┐
│          Aether Launcher           │        │           Aether Client          │
│   Rust core + native shell (Tauri)  │        │            (Fabric mod)          │
│                                    │        │                                  │
│  Rust core (src/*.rs, CLI/JSON):   │        │  AetherClient / Aether           │
│   - accounts, settings, paths      │        │  ModuleManager / HudManager      │
│   - minecraft metadata & rules     │  args  │  Click GUI / Config / HUD editor │
│   - installer (download/verify)    │───────►│  Mixins + session/config logic   │
│   - game (classpath & launch)      │classpath│                                  │
│                                    │        │                                  │
│  Shell (desktop/src/*.rs):         │        │                                  │
│   - window, single instance        │        │                                  │
│   - runs the core, streams events  │        │                                  │
│   - serves gui/src/ui (WebView2)   │        │                                  │
└────────────────────────────────────┘        └──────────────────────────────────┘
```

The launcher prepares and starts a Minecraft/Fabric process; the mod runs inside that process and
provides the client-side features.

## Launcher architecture

### Rust core (`launcher/src`)

The core contains all business logic and exposes a command-line interface (`main.rs`) that emits
JSON lines/events for the interface.

| Module | Responsibility |
| --- | --- |
| `main.rs` | CLI parsing/dispatch, JSON output, streaming wrappers for install/launch/auth flows. |
| `lib.rs` | High-level `Launcher` API tying paths, settings, accounts, install, launch and the game record together. |
| `paths.rs` | Resolution/creation of launcher and game directories (`AETHER_HOME`, `AETHER_SHARED_DIR`). |
| `settings.rs` | Persistent settings (including the Identity Studio colourway, custom accent, surface, density and navigation choices), Java detection, mod jar discovery (`AETHER_MOD_JAR`). |
| `accounts.rs` | Account store, offline accounts, Microsoft device-code login flow, session mapping, and the Mojang name lookup that tells a premium name from a free one. |
| `credentials.rs` | Separate Microsoft access/refresh token vault: Windows DPAPI protection, legacy account-file migration, and no token serialization into JSON/renderer IPC. |
| `offline.rs` | Offline username validation and deterministic Minecraft-compatible offline UUID generation. |
| `minecraft.rs` | Mojang/Fabric metadata retrieval, rule evaluation, argument templating, library resolution/merging. |
| `http.rs` | HTTP helpers, JSON/text/bytes requests with status (`get_with_status`), file download with checksum verification, SHA-1 utilities. |
| `installer.rs` | Version installation pipeline (client, libraries, assets, Fabric API, natives, mod copy) with progress reporting. |
| `game.rs` | Classpath construction, JVM/game argument construction, process launch, output forwarding, and the record of the running game (`game.pid`). |
| `servers.rs` | Saved server list, Server List Ping, login probe that tells an offline-accepting server from a premium-only one. |
| `hosting.rs` | Local Fabric server: shared installer, `server.properties`, RCON shutdown, process liveness (also provides the process probe and the tree killer the game uses). |
| `performance.rs` | Optimisation pack resolved on Modrinth and verified by SHA-1, `options.txt` presets with backup, manifest of what was installed. |

Key properties:
- **Single source of truth for launch/install logic** in Rust; the interface does not duplicate it.
- **JSON CLI contract** enables scripting and decouples the interface from the core.
- **Checksum-verified downloads** (SHA-1 where provided/expected) to avoid corrupted/cached-bad files.
- **Untrusted metadata stays inside its namespace**: version IDs, library-relative paths, asset hashes,
  hosted-server version identifiers and remote JAR filenames are validated before local path joins.
- **Content-addressed asset scheduling**: duplicate SHA-1 objects are collapsed before parallel
  downloads, preventing workers from racing over a shared object destination or partial file.
- **Managed file boundaries**: mod and session-log operations reject symbolic links instead of
  following them outside their managed folders.

### Native shell (`launcher/desktop`)

Tauri 2 serves the interface to the system WebView2 runtime. There is no bundled browser: the shell
is a single ~8.75 MB executable, the backend is the same language as the core, and the interface files
are the same ones the previous Electron shell loaded.

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Window creation (shown only once the first frame is ready), single-instance lock, command registration, kill of the running processes on exit. |
| `src/commands.rs` | One `#[tauri::command]` per call of the interface: same names, same arguments and same answers as the old IPC handlers. |
| `src/core.rs` | Finds, embeds and runs the core; turns its JSON lines into events (`install:progress`, `game:output`, …); enforces a per-command timeout; atomically extracts embedded binaries/JARs with unique temporary siblings; exposes `kill_tree`. |
| `src/state.rs` | The processes the window can stop while they run (installation, game), with atomic single-operation guards so concurrent commands cannot overwrite the tracked PID. |
| `tauri.conf.json` | Window size/behaviour, injected CSP and JavaScript prototype freeze, NSIS packaging (French), icons and licence. |

The interface (`launcher/gui/src/ui/`) is plain HTML/CSS/JavaScript. `bridge.js` exposes
`window.aether` over Tauri's `invoke`, so `app.js` never learned that the shell changed.

The Settings page is **Identity Studio**: app.js applies persisted workspace choices to root-level
palette aliases, surface, density and navigation attributes; tokens.css is the single source for
all four material colourways; styles.css renders those choices across the actual shell and the
live workspace preview. Appearance writes are serialized so rapid changes cannot overwrite one
another with stale settings snapshots.

The WebView policy is set in `tauri.conf.json`, not only in an HTML meta tag: scripts are self-hosted,
objects are disabled, the base URL and form targets are restricted to the local origin, and
`freezePrototype` prevents page scripts from modifying built-in JavaScript prototypes.

**Why the shell is native and not Electron.** The Electron build shipped a whole Chromium for a
launcher: ~150 MB installed against one ~8.75 MB executable, and it added a Node runtime next to the
Rust core for no benefit (the interface calls the core over JSON either way). The cost of the move
was the shell only — not the interface, not the core, not the module system.

### The record of the game (`game.pid`)

The shell holds the child it started and kills it when the window closes. That covers the normal
path, not the abnormal one: a shell killed from the task manager (or a game started from the command
line) would leave Java running with nothing able to name it.

`game::launch` therefore calls back with the process identifier as soon as Java exists, and
`Launcher::launch` writes it to `<home>/aether/game.pid`. The record is:

- removed when the game exits, whatever the exit code;
- read by `Launcher::running_game()` / `game::running()`, which clear a record that names a dead
  process instead of trusting it;
- used by `Launcher::stop_game()` / `game::stop()`, which kills the tree (`taskkill /T /F`, or
  `SIGTERM` elsewhere) and waits for the process to disappear before deleting the record;
- exposed as `aether-launcher stop`, and as the fallback of the shell's `game_stop`: when the
  in-memory slot is empty, the command runs the core's `stop` instead of answering "no game is
  running" while an orphan keeps running.

## Launcher data flow

1. **Open/load**: resolve paths → load settings/accounts → detect/configure Java if needed.
2. **Install**: resolve metadata (vanilla + Fabric profile) → download/verify artifacts → place mod/API jars → prepare natives/assets/indexes.
3. **Launch**: build merged version/classpath/arguments for selected account and profile → spawn Java process → write `game.pid` → stream logs/exit code → clear `game.pid`.
4. **Stop**: the shell kills its own child; `stop` kills the recorded tree, which is the only path left after a shell crash.
5. **Servers**: saved list → ping (`SLP`) → login probe (`Login Start` and the first answer of the server) → join with `--quickPlayMultiplayer`.
6. **Hosting**: Fabric installer (downloaded once, shared) → `server.properties` with `online-mode=false` and RCON → start → wait for the ping → stop through RCON, then kill the tree if the server hangs.
7. **Performance**: resolve each slug on Modrinth for the game version → download and verify the SHA-1 → write the manifest → write `options.txt` (backup first) → pick the JVM flags for the next launch.

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

### Premium-name guard

The probe answers "does this server verify sessions with Mojang". It cannot answer the next question:
a cracked server accepts offline sessions but **reserves the names Mojang owns**, so `Notch` is
disconnected with `Unauthorized: You cannot join the server` while `Notch9281` joins in the same
second. The game logs neither the reason nor the name, and no client can negotiate the rule away.

`accounts::name_status` therefore asks Mojang before the launch:

```text
GET https://api.mojang.com/users/profiles/minecraft/<name>
200 + profile  -> premium (a Mojang account owns it)
204 / 404      -> free
other status   -> error, never read as "free"
```

`http::get_with_status` exists for this call: it returns `(status, body)` and only raises the codes
the retry loop is meant to see (`408`, `425`, `429`, `5xx`). A rate limit cannot turn into a false
"this name is free". The command is `account check <name>`; the interface calls the same command
through `accounts_check_name` and uses the answer twice: live under the name field (debounced 450 ms,
out-of-order answers dropped) and once more before joining a non-local server. `Join anyway` is
offered, and a failed lookup lets the launch continue: the guard informs, it does not decide for the
player.

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
| `module/` | Module framework, categories, registry, lifecycle callbacks and per-callback fault isolation. |
| `module/modules/` | Concrete modules (HUD/render/combat/movement/misc). |
| `setting/` | Typed module settings and UI-editable values. |
| `hud/` | HUD element management, layout/position persistence, rendering integration. |
| `gui/` | Click GUI, config screen, HUD editor and rendering helpers. |
| `util/` | Shared utilities (render/color/animation/session/click tracking, bar chart, etc.). |
| `mixin/` | Targeted game patches where Fabric hooks/events are insufficient. |

### Mixins (principle)

Mixins are kept minimal and used only when no suitable API hook exists (e.g. UI replacement,
input/click capture, HUD/chat behavior adjustments). Tests in `src/test/java` validate mixin/config
consistency against the target game classes where applicable.

## Configuration and persistence

- **Launcher**: settings and accounts stored under the launcher data directory (e.g. `<home>/Aether/aether`).
- **Running game**: `<home>/Aether/aether/game.pid`, written at spawn and removed at exit.
- **Client**: module/HUD/config state stored in `config/aether/config.json` inside the game/config context.
- **Game files**: assets, libraries, versions and mods are installed in the configured game directory.
- **Servers**: the saved list lives in `aether/servers.json`, each hosted server owns a directory under
  `<home>/servers/<name>` with its own `server.properties`, world and `console.log`.
- **Performance**: the record of the installed mods is `aether/performance.json`; the first tuning
  writes `options.txt.aether-backup` next to the game options.

The client configuration tolerates a UTF-8 byte order mark and uses schema version 1 (unversioned files
are migrated in place). Writes are staged in unique sibling files, flushed and promoted with an atomic
replace when the file system supports it. Before replacing a valid primary, the previous bytes are
atomically written to `config.json.bak`; a corrupt primary is preserved under a unique
`.corrupt-*.json` name before a successful replacement. Startup tries the primary, then the backup,
and otherwise uses defaults while preserving both files. A recovery notice is shown in-game when a
backup is restored or defaults are required.

A small session guard records whether Fabric reached the normal client-stopping lifecycle event.
Two consecutive unclean starts produce a notice that offers the player the `/aether safe-mode`
command. The marker says only that the previous sessions did not close normally; it is not treated
as proof that Aether caused a Minecraft crash. Safe mode disables Aether modules for that session,
keeps the saved module preferences unchanged, and suppresses GUI/keybind toggles until the next
normal launch.

Module `onTick` and `onRender` callbacks run behind a narrow `RuntimeException` boundary. Three
failures from the same callback within ten seconds trip that module's circuit breaker; successful
calls reset that callback's failure window. Lifecycle callback failures are logged and an unsuccessful
enable is rolled back with a best-effort disable cleanup. JVM `Error` types are deliberately not caught.
This can attribute failures thrown by Aether callbacks, but does not claim to diagnose arbitrary
Minecraft or JVM crashes.

## Build/test components

- **Launcher core**: Cargo project (`launcher/Cargo.toml`), Rust tests under `tests/` and in each module.
- **Shell**: Cargo project (`launcher/desktop/Cargo.toml`, Tauri 2), NSIS packaging through
  `cargo tauri build`; the core binary is embedded at compile time, so the core is built first.
- **Client**: Gradle + Fabric Loom build (`build.gradle`, `gradle.properties`), JUnit tests under `src/test/java`.
- **Interface**: `launcher/gui/src/ui/` (HTML/CSS/JavaScript), no build step, no npm dependency.

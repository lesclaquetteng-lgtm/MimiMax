# Verification

How the current implementation of **Aether Client** and **Aether Launcher** is verified, and the
results of the last full run (14 November 2026, Minecraft 1.21.11 / Fabric 0.20.1).

> Everything below was measured on this machine, not estimated. The console excerpts come from the
> logs written by the core, the game and the hosted server during the run.

## 0. Results at a glance

| Check | Command | Result |
| --- | --- | --- |
| Launcher tests | `cargo test --release` | **151 passed, 0 failed** (122 unit, 6 download, 20 integration, 3 SHA-1) |
| Launcher lints | `cargo clippy --release --all-targets` | **0 warning** |
| Release binary | `cargo build --release` | 2.1 MB |
| Mod tests | `gradle build` | **25 passed, 0 failed** (5 suites) |
| Mod jar | `gradle build` | `aether-client-1.0.0.jar`, 118.9 KB |
| Interface layout | `node tools/ui-check.mjs` | **16 checks, 0 overflow, 0 offending element** |
| Real server ping | `servers ping mc.hypixel.net` | online, 26 863 / 200 000, 458 ms, protocol 774 |
| Offline probe, premium server | `servers probe mc.hypixel.net` | `premium`, first packet `encryption request` |
| Offline probe, hosted server | `servers probe 127.0.0.1:25565` | `offline` accepted, first packet `set compression` |
| Hosted server start | `hosts start "Local Test"` | ready in **6–8 s**, `Done (0.370s)!` |
| Hosted server stop | `hosts stop "Local Test"` | **726 ms**, chunks saved, RCON listener stopped |
| Game with the full pack | `launch --server 127.0.0.1:25565` | joined as `Notch`, **889 FPS** in the HUD, 1044 FPS peak |
| Options preset after a session | `performance tune --preset max` | `options.txt` **identical** after the game closed |

## 1. Offline UUID compatibility (critical)

**Goal:** the offline identity must match Minecraft's offline UUID derivation.

**Rule:**
`UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(StandardCharsets.UTF_8))`

**Reference values**
- `Notch`      → `b50ad385-829d-3141-a216-7e7d7539ba7f`
- `Steve`      → `5627dd98-e6be-3c21-b8a8-e92344183641`
- `Player123`  → `3da1e349-17b0-38ed-a07b-1e78c8ebd956`

**Where it is enforced**
- `launcher/src/offline.rs` (unit tests compare against the reference values)
- `aether-client/src/test/java/dev/aether/client/OfflineSessionTest.java`
- `launcher/src/lib.rs` passes `--username`, the deterministic `--uuid` and `--accessToken 0`

The last launch log contains `Aether Client 1.0.0 ready - session: Offline`, and the hosted server
answered `Notch joined the game`: the identity sent by the launcher is accepted by a real server.

## 2. Launcher core (Rust)

```powershell
cd launcher
cargo test --release
cargo clippy --release --all-targets
cargo build --release
```

Covered by the suite: settings and accounts, path resolution, metadata and rule evaluation, library
and argument resolution, download/checksum primitives, the installer pipeline, the server list, the
login probe parsing, the hosting properties/RCON helpers, and the options presets.

Two tests were added during this run because the code had a real defect:

- `the_pack_lists_unique_slugs_and_carries_its_libraries` — the pack must ship Cloth Config,
  otherwise MoreCulling and Dynamic FPS refuse to load and the game does not start.
- `the_presets_of_the_options_stay_inside_the_ranges_of_the_game` — `biomeBlendRadius` and
  `maxAnisotropyBit` must stay in the range the game accepts, the particle setting must be 0–2, and
  `graphicsPreset` must be `custom` (a named preset rewrites the tuned values at start up).

## 3. Client mod (Fabric / Gradle)

```powershell
cd aether-client
gradle build
```

- 25 JUnit tests: configuration round trip, module registry, legacy configuration migration, mixin
  configuration, offline session, utilities.
- A test was added for the UTF-8 byte order mark: Notepad writes one, Gson refuses it, and the whole
  configuration used to fall back to the defaults.
- The built jar is remapped by Loom and copied to the mods folder by `aether-launcher install`.

## 4. Interface (native shell)

The interface is plain HTML/CSS/JavaScript, served by the Tauri shell to WebView2. It talks to the
core through `gui/src/ui/bridge.js`, which exposes the same `window.aether` object the Electron
preload used to define.

```powershell
cargo test --release --manifest-path desktop/Cargo.toml
cargo build --release --manifest-path desktop/Cargo.toml
```

The shell tests cover the shape of the answers it builds, the JSON documents of the core and the
timeouts of the network commands. The window itself is covered by the checks below.

### 4.0 What the shell guarantees

- **One instance**: a second launch focuses the window that is already open, so two windows cannot
  write the same settings.
- **Embedded core**: `desktop/src/core.rs` embeds `target/release/aether-launcher.exe`, writes it to
  the application data on first start and replaces it when its bytes change.
- **No orphan on close**: `RunEvent::ExitRequested` / `Exit` kill the installation and the game that
  the window started (`state::kill_active`).
- **No orphan after a crash**: the core records the game in `<home>/aether/game.pid`, so `stop` (and
  the Stop button of the next session) still kills the tree of a launcher that was killed.

### 4.1 Layout and visual probe

The check drives the real application through CDP, on an isolated `AETHER_HOME`:

```powershell
$env:AETHER_HOME = "tools/ui-home"
$env:AETHER_SHARED_DIR = "tools/aether-home/.minecraft"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9233"
& launcher/desktop/target/release/aether-desktop.exe
node tools/ui-check.mjs 9233 tools/ui-shots
```

What is measured on every page, at **1180×760** and **940×620**:

- `documentOverflow` — the document must not scroll sideways;
- `contentOverflow` — the content column must not scroll sideways;
- `offenders` — no element whose content is wider than its own box (a cut label), ignoring the
  elements that end with an ellipsis on purpose.

| Size | Checks | Document overflow | Content overflow | Offenders |
| --- | --- | --- | --- | --- |
| 1180×760 | 7 pages | 0 | 0 | 0 |
| 940×620 | 9 checks | 0 | 0 | 0 |

The pages showed real data read from the core: 1 saved server (Hypixel), 1 hosted server
(`Local Test`, stopped), **11/11** optimisation mods, JVM preset `ultra`, options preset `max`. The
verdict of the probe was displayed on the page: *Premium only: the server runs in online mode, a
Microsoft account is required. 26 819/200 000 players · 397 ms.*

Screenshots: [`images/launcher-servers.png`](images/launcher-servers.png),
[`images/launcher-performance.png`](images/launcher-performance.png),
[`images/launcher-play.png`](images/launcher-play.png), and the narrow variants
(`*-narrow.png`).

### 4.2 End-to-end checks (`tools/e2e/`)

The behavioural checks live in `tools/e2e/`, **not** in `target/`: `cargo clean` must not be able
to delete them. They drive the real application over CDP; the launcher is started with a fake core
so the failures are reproducible without touching the network:

These scripts drive the real window and the real Rust core on an isolated data home. The
`profiles.mjs` walk creates an offline account through the Accounts dialog, then exercises profile
selection, editing, creation and deletion through the Play page: it covers the complete
renderer → bridge → command → core path, not only the helper functions. They are no longer part of
CI (the shell needs a desktop session): run them locally with the commands below.


```powershell
# Build the fake cores once (they only use the standard library).
rustc -O tools/e2e/fake_core.rs -o tools/e2e/fake-core.exe
rustc -O tools/e2e/hang.rs -o tools/e2e/hang.exe

# Start the launcher on an isolated home, with the fake core and a debugging port.
$env:AETHER_HOME = "tools/e2e/home"
$env:AETHER_LAUNCHER_BIN = "tools/e2e/fake-core.exe"
$env:AETHER_REAL_CORE = "launcher/target/release/aether-launcher.exe"
$env:AETHER_FAKE_FAIL = "install,launch-exit"   # the failure mode of the run
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"
& launcher/desktop/target/release/aether-desktop.exe

# Then, from launcher/:
npm run e2e:failure -- 9222 documents   # install + launch error documents, installation report
npm run e2e:crash -- 9222 game          # crash report, copy, and the "Open the logs" link
npm run e2e:drive -- 9222 <pid-file>    # the full walk: install, launch, stop, settings
```

| Script | What it proves |
| --- | --- |
| `drive.mjs` | the whole interface against the real IPC: paths, accounts, install, launch, stop |
| `screens.mjs` | every page at two sizes, no overflow, screenshots |
| `failure.mjs` | error documents: toasts, controls given back, installation report, retry |
| `crash-report.mjs` | the crash report opens, copies, and leads to the session in the logs |
| `cancel.mjs` | a slow installation is cancelled, the `.part` files stay for the resume |
| `stubborn.mjs` | a core that ignores `cancel` is killed after the grace period |
| `timeout.mjs` | a core that never answers is killed and reported |
| `a11y.mjs` | the dialog roles, the focus trap, Escape, the announced toasts |
| `profiles.mjs` | named profiles: create, assign, switch, remove |
| `sliders.mjs` | a real drag updates the value and the `--fill` of every slider |

## 5. Offline servers

### 5.1 A premium server says so

```text
aether-launcher servers ping mc.hypixel.net
  online, 26 863/200 000, 458 ms, protocol 774
aether-launcher servers probe mc.hypixel.net
  verdict "premium", packet "encryption request"
```

### 5.2 A hosted server accepts an offline account

```text
aether-launcher hosts start "Local Test"
  ready in 6 s, server.pid 24148, rconPort 1783
console.log:
  [Server thread/WARN]: **** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!
  [Server thread/INFO]: Done (0.370s)! For help, type "help"
  [Server thread/INFO]: RCON running on 0.0.0.0:1783

aether-launcher servers probe 127.0.0.1:25565
  verdict "offline", accepted, packet "set compression"
```

### 5.3 Joining it from the launcher

```text
aether-launcher launch --server 127.0.0.1:25565
client log : Connecting to 127.0.0.1, 25565
server log : Notch[/127.0.0.1:57215] logged in with entity id 34
             Notch joined the game
```

### 5.4 Stopping it saves the world

```text
aether-launcher hosts stop "Local Test"     (726 ms)
  ThreadedAnvilChunkStorage (world): All chunks are saved
  ThreadedAnvilChunkStorage: All dimensions are saved
  Thread RCON Listener stopped
```

## 6. Maximum FPS

The pack was installed from Modrinth and verified by SHA-1 (11/11): Sodium 0.8.15, Lithium 0.21.4,
FerriteCore 8.2.0, ImmediatelyFast 1.14.3, EntityCulling 1.12.1, MoreCulling 1.6.2, Cloth Config
21.11.153, Krypton 0.2.10, BadOptimizations 2.4.1, Dynamic FPS 3.11.6, Clumps 29.0.0.1.

Measured in game on a RTX 4060 Ti, Ryzen 7 5700X, Java 21, window 854×480, render distance 8:

| Situation | Frame rate |
| --- | --- |
| Debug overlay (F3) just after joining | **653 FPS** (`p50=692`, `p2=402`) |
| Clean HUD, options preset `max` | **889 FPS** |
| Peak seen during the session | **1044 FPS** |

Evidence: [`images/fps-debug-sodium.png`](images/fps-debug-sodium.png) (F3, Sodium renderer,
ImmediatelyFast) and [`images/fps-offline-hud.png`](images/fps-offline-hud.png) (Aether HUD, offline
server, 889 FPS).

The tuned options survive a full session: `options.txt` was compared byte by byte before and after
the game closed, with no difference. That is what `graphicsPreset:"custom"` buys — with `fast`, the
game re-applied its own bundle at start up and reset the ambient occlusion, the clouds, the particles
and the entity distance.

## 7. Defects found by these runs, and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| The game refused to start: `Incompatible mods found!` | MoreCulling and Dynamic FPS require Cloth Config, which was not in the pack | `cloth-config` added to the pack, plus a test |
| `Error parsing option value 0 for option Anisotropic Filtering` | `maxAnisotropyBit:0` is outside `[1:4]` | Presets corrected, plus a range test |
| Half of the tuning was silently undone | `graphicsPreset:"fast"` re-applies the whole Fast bundle at start up | `graphicsPreset:"custom"` |
| The settings of the client were lost | A UTF-8 byte order mark made Gson refuse the file | The marker is stripped before parsing, plus a test |
| The FPS graph was drawn under the watermark | Both elements claimed the top right corner | The graph sits at `screenWidth - 124, 22` |

## 8. Known verification constraints

- Network access is required for metadata, artifacts and the Modrinth resolution.
- Microsoft login needs an interactive device-code flow and a real entitlement.
- An offline account cannot join an `online-mode=true` server; this is a property of Minecraft, not a
  bug, and the Servers page says so before the game starts.
- The frame rates above depend on the machine, the window size and the render distance; the options
  preset and the mod list are the reproducible part.

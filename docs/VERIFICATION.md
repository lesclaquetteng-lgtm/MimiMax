# Verification

How the current implementation of **Aether Client** and **Aether Launcher** is verified, and the
results of the last full run (19 November 2026, Minecraft 1.21.11 / Fabric 0.20.1).

> Everything below was measured on this machine, not estimated. The console excerpts come from the
> logs written by the core, the game and the hosted server during the run.

## 0. Results at a glance

| Check | Command | Result |
| --- | --- | --- |
| Launcher tests | `cargo test --release` | **163 passed, 0 failed** (134 unit, 6 download, 20 integration, 3 SHA-1) |
| Launcher lints | `cargo clippy --release --all-targets` | **0 warning** |
| Release binary | `cargo build --release` | 2.1 MB |
| Mod tests | `gradle build` | **87 passed, 0 failed** (16 suites) |
| Mod jar | `gradle build` | `aether-client-1.0.0.jar`, 159.8 KB |
| Interface layout | `node tools/ui-check.mjs` | **16 checks, 0 overflow, 0 offending element** |
| Real server ping | `servers ping mc.hypixel.net` | online, 26 863 / 200 000, 458 ms, protocol 774 |
| Offline probe, premium server | `servers probe mc.hypixel.net` | `premium`, first packet `encryption request` |
| Offline probe, hosted server | `servers probe 127.0.0.1:25565` | `offline` accepted, first packet `set compression` |
| Cracked server ping | `servers ping play.jartexnetwork.com` | online, **658 / 5 000**, 1 466 ms, `Velocity 1.7.2-26.3`, protocol 774 |
| Cracked server probe | `servers probe play.jartexnetwork.com` | `offline` accepted, first packet `set compression` |
| Premium name on a cracked server | `launch --server play.jartexnetwork.com` as `Notch` | refused: `Unauthorized: You cannot join the server` |
| Free name on the same server | same launch, account `AetherTester9281` | **joined** the Prison lobby |
| Hosted server start | `hosts start "Local Test"` | ready in **6–8 s**, `Done (0.370s)!` |
| Hosted server stop | `hosts stop "Local Test"` | **726 ms**, chunks saved, RCON listener stopped |
| Game with the full pack | `launch --server 127.0.0.1:25565` | joined as `Notch`, **889 FPS** in the HUD, 1044 FPS peak |
| Options preset after a session | `performance tune --preset max` | `options.txt` **identical** after the game closed |
| Installer, silent | `Aether-Launcher-Setup-1.0.0.exe /S` | exit 0 in **1.6 s**, shortcuts + uninstall entry written |
| Installer, five pages | `tools/installer-capture.ps1` | welcome 0.2 s, licence 1.3 s, folder 2.4 s, install 4.8 s, finish 5.5 s |
| Uninstaller | `uninstall.exe /S` | exit 0 in **1.3 s**, folder gone in 1 s |
| Clean machine, no sources | `tools/clean-install-check.ps1` | core alone: `the Aether client jar was not found`; shell: mod extracted (SHA-256 match), installation starts |

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

Microsoft credential handling is covered by the account serialization and credential-vault tests:

- The public account catalogue does not serialize access or refresh tokens, so account-list JSON and
  the renderer IPC response cannot expose those fields.
- On Windows, the DPAPI round-trip test writes protected ciphertext, reloads it for the same user and
  restores the session fields in memory.
- Legacy plaintext account files are migrated by writing the protected vault before replacing the
  public account catalogue. A failed migration scrubs the legacy fields from the catalogue and asks
  the player to sign in again.
- The diagnostic command masks the access token before printing launch arguments.

- Remote version identifiers, Fabric versions, Maven artifact paths, asset index identifiers and
  asset SHA-1 names are checked before they are used in local paths.
- Modrinth JAR filenames reject directory separators, NTFS alternate data streams, control
  characters and reserved Windows device names.
- Native ZIP extraction rejects entries that are not enclosed by the extraction root.
- Asset-index entries are deduplicated by SHA-1 before parallel downloads; conflicting sizes for
  the same content hash are rejected instead of racing over one destination and partial file.
- Hosted-server Minecraft/Fabric version identifiers are validated before they become shared
  artifact-directory names.
- Mod and session-log discovery/reads refuse symbolic links rather than following them outside the
  managed directories.
- Atomic file writes use unique sibling temporary files; concurrent writes of accounts.json and
  accounts.credentials are covered by a regression test.


Two tests were added during this run because the code had a real defect:

- `the_pack_lists_unique_slugs_and_carries_its_libraries` — the pack must ship Cloth Config,
  otherwise MoreCulling and Dynamic FPS refuse to load and the game does not start.
- `the_presets_of_the_options_stay_inside_the_ranges_of_the_game` — `biomeBlendRadius` and
  `maxAnisotropyBit` must stay in the range the game accepts, the particle setting must be 0–2, and
  `graphicsPreset` must be `custom` (a named preset rewrites the tuned values at start up).

Three more lock the premium-name lookup, which is what a cracked server refuses:

- `a_name_owned_by_a_mojang_account_is_read_as_premium` — a `200` profile means the name is taken.
- `a_name_nobody_owns_is_not_premium` — both `204` and `404` mean the name is free.
- `a_broken_lookup_is_never_read_as_a_free_name` — a `429`/`5xx` is an error, so a rate limit cannot
  be mistaken for "this name is available".

## 3. Client mod (Fabric / Gradle)

```powershell
cd aether-client
gradle build
```

- 87 JUnit tests (16 suites): configuration round trip, module registry, legacy configuration
  migration, mixin configuration, offline session, text fitting, the HUD modules and their
  lifecycles, the knockback filters, the render utilities, the catalogue and the language coverage.
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
timeouts of the network commands. Regression coverage also checks that embedded-resource writes
use unique temporary siblings and atomic replacement, concurrent extraction never leaves partial
files, and only one installation/game launch can own the corresponding process slot at a time.
The window itself is covered by the checks below.

### 4.0 What the shell guarantees

- **One instance**: a second launch focuses the window that is already open, so two windows cannot
  write the same settings.
- **Embedded core**: `desktop/src/core.rs` embeds `target/release/aether-launcher.exe`, writes it to
  the application data on first start and replaces it when its bytes change.
- **Embedded mod**: the same file embeds `aether-client/build/libs/aether-client-1.0.0.jar` and
  extracts it to `%LOCALAPPDATA%\dev.aether.launcher\mods\aether-client.jar`, which it passes to the
  core as `AETHER_MOD_JAR`. Without it, an installed launcher cannot answer "Install / repair": the
  core would look for `aether-client/build/libs/…` on a machine that never had the sources.
  `build.rs` refuses to build the shell when the jar is missing.
- **No overlapping processes**: atomic operation guards prevent a second installation or game launch
  from replacing the PID/stdio handle tracked by the shell while the first operation is still active.
- **No orphan on close**: `RunEvent::ExitRequested` / `Exit` kill the installation and the game that
  the window started (`state::kill_active`).
- **Atomic embedded resources**: the embedded core and JAR are written to unique temporary siblings,
  synced, and replaced atomically; a locked old executable is preserved rather than truncated.
- **WebView policy**: the Tauri configuration injects a restrictive CSP (`object-src 'none'`,
  `base-uri 'self'`, `form-action 'self'`) and freezes JavaScript built-in prototypes.
- **No orphan after a crash**: the core records the game in `<home>/aether/game.pid`, so `stop` (and
  the Stop button of the next session) still kills the tree of a launcher that was killed.

Measured on the installed build (`%LOCALAPPDATA%\Aether Launcher\aether-desktop.exe`), from an empty
folder and an empty home:

```text
control (core alone, no AETHER_MOD_JAR) : {"ok":false,"error":"the Aether client jar was not found at
                                          aether-client/build/libs/aether-client-1.0.0.jar"}
mod extracted                          : aether-client.jar, 163 592 B, SHA-256 6BF21897…
                                         (equal to aether-client/build/libs/aether-client-1.0.0.jar)
install                                : "Resolving version metadata", then "Downloading Minecraft client"
```

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
# Rebuild the two fake cores when their sources change (standard library only).
rustc -O tools/e2e/fake_core.rs -o tools/e2e/fake-core.exe
rustc -O tools/e2e/hang.rs -o tools/e2e/hang.exe

# Replay every scenario from the repository root: one window, one port and one
# throwaway home per check.
powershell -NoProfile -File tools/e2e/replay.ps1

# One check only, when iterating.
powershell -NoProfile -File tools/e2e/replay.ps1 -Only failure,crash
```

`replay.ps1` holds the matrix itself (`tools/e2e/replay.ps1:11-23`); the ports are shifted by a
random multiple of a hundred so a window left open by an earlier run cannot answer in their place:

| Check | Script | Port | Core | `AETHER_FAKE_FAIL` | Data home |
| --- | --- | --- | --- | --- | --- |
| `a11y` | `a11y.mjs` | 9222 | fake | — | empty |
| `sliders` | `sliders.mjs` | 9223 | fake | — | empty |
| `screens` | `screens.mjs` | 9224 | fake | — | account, captures in `tools/e2e/shots-replay/` |
| `drive` | `drive.mjs` | 9225 | fake | — | account, pid file `tools/e2e/pids-replay.txt` |
| `profiles` | `profiles.mjs` | 9226 | **real** | — | hardcore profile |
| `failure` | `failure.mjs` | 9227 | fake | `install,launch` | account |
| `crash` | `crash-report.mjs` | 9228 | fake | `launch-exit` | account |
| `cancel` | `cancel.mjs` | 9229 | fake | `install-slow` | account |
| `stubborn` | `stubborn.mjs` | 9230 | fake | `install-stubborn` | account |
| `timeout` | `timeout.mjs` | 9231 | hang | — | empty |
| `hit` | `hit.mjs` | 9232 | fake | — | account |

Three things the matrix does not say by itself:

- **`AETHER_REAL_CORE` is mandatory.** The fake core fakes install and launch only; it forwards
  accounts and profiles to the real launcher, which `replay.ps1` points at
  `launcher/target/release/aether-launcher.exe`. Without that binary the fake core panics
  (`AETHER_REAL_CORE is set: NotPresent`) before any check runs.
- **The seed is part of the check.** `account` writes an offline account, three mods
  (`aether-client.jar`, `sodium.jar`, `iris.jar.disabled`) and one finished session log into the
  home; `hardcore` adds a profile of 8192 MB instead. A check that needs a player fails on an
  empty home.
- **Every home is throwaway.** Each case gets `tools/e2e/home-replay-<case>-<stamp>`, removed at the
  end of the run; `guard.mjs` (and the same rule inside `replay.ps1`) refuses any home outside
  `tools/e2e/`, and `.gitignore` covers `home-replay-*`, `shots-replay/` and `pids-replay.txt`.

The eight scripts that click or drag go through `click.mjs`, which scrolls the target into view and
checks the point with `document.elementFromPoint` before sending any event: a click that would land
on a toast or a dialog stops at once with the name of what is in the way. `hit.mjs` proves that guard
in both directions, and `node --test tools/e2e/click.test.mjs` keeps every press on that path.

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
| `hit.mjs` | the click guard: a covered button is refused without any event, a child counts, off-screen/hidden/disabled/missing are named |

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

## 6. Cracked servers and premium names

The failure reported from the interface was "I cannot connect to online servers such as JartexNetwork".
The server itself is not the problem:

```text
aether-launcher servers ping play.jartexnetwork.com
  online, 658/5000, 1466 ms, protocol 774, Velocity 1.7.2-26.3
aether-launcher servers probe play.jartexnetwork.com
  verdict "offline", accepted, packet "set compression"
```

Two launches of the same client, on the same server, with two offline accounts:

| Account | Name | Result |
| --- | --- | --- |
| `Notch` | owned by a Mojang account | `Failed to connect to the server` — `Unauthorized: You cannot join the server` |
| `AetherTester9281` | owned by nobody | **joined** the Prison lobby |

Evidence: [`images/jartex-premium-kick.png`](images/jartex-premium-kick.png) and
[`images/jartex-joined.png`](images/jartex-joined.png).

The client was never at fault. JartexNetwork protects the names Mojang owns: it accepts offline
sessions, then refuses any of them that presents a premium name, to protect that name's owner. The
game shows a single line, without naming the name, and no client can negotiate the rule away.

The launcher now answers the question before the game starts:

```text
aether-launcher account check Notch
  {"name":"Notch","ok":true,"premium":true}
aether-launcher account check AetherTester9281
  {"name":"AetherTester9281","ok":true,"premium":false}
aether-launcher account check xq7ztnowhere
  {"name":"xq7ztnowhere","ok":true,"premium":false}
```

In the interface, the same lookup drives two guards, both captured from the shipped build:

- under the name field, after a 450 ms pause, an amber message explains that the name belongs to a
  Mojang account and suggests a free variant ([`images/launcher-premium-input.png`](images/launcher-premium-input.png));
- before joining a public server, a dialog titled *This name belongs to a Mojang account* offers
  `Choose another name` or `Join anyway` ([`images/launcher-premium-name.png`](images/launcher-premium-name.png)).

Loopback and `localhost` addresses are skipped (a server you host accepts any name), `Join anyway`
stays available, and a lookup that fails never blocks a launch. The probe of the server and the
lookup of the name are two different questions: the first says the server accepts offline sessions,
the second says this name is allowed to use one.

## 7. Maximum FPS

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

## 8. Defects found by these runs, and fixed

| Symptom | Cause | Fix |
| --- | --- | --- |
| The game refused to start: `Incompatible mods found!` | MoreCulling and Dynamic FPS require Cloth Config, which was not in the pack | `cloth-config` added to the pack, plus a test |
| `Error parsing option value 0 for option Anisotropic Filtering` | `maxAnisotropyBit:0` is outside `[1:4]` | Presets corrected, plus a range test |
| Half of the tuning was silently undone | `graphicsPreset:"fast"` re-applies the whole Fast bundle at start up | `graphicsPreset:"custom"` |
| The settings of the client were lost | A UTF-8 byte order mark made Gson refuse the file | The marker is stripped before parsing, plus a test |
| The FPS graph was drawn under the watermark | Both elements claimed the top right corner | The graph sits at `screenWidth - 124, 22` |
| A cracked server refused the connection with only `Unauthorized: You cannot join the server` | The offline account used `Notch`, a name Mojang owns, which the server protects; the game names neither the reason nor the name | `account check` asks Mojang, the name field warns after 450 ms, and joining a public server opens a dialog with `Choose another name` / `Join anyway` |

## 9. Known verification constraints

- Network access is required for metadata, artifacts and the Modrinth resolution.
- Microsoft login needs an interactive device-code flow and a real entitlement.
- An offline account cannot join an `online-mode=true` server; this is a property of Minecraft, not a
  bug, and the Servers page says so before the game starts.
- A cracked server accepts an offline session but may refuse a name a Mojang account owns. The guard
  explains it before the launch; it cannot make the server accept the name, and it does not try.
- The frame rates above depend on the machine, the window size and the render distance; the options
  preset and the mod list are the reproducible part.

# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- A hosted server now writes `white-list=true`, `enforce-whitelist=true` and
  `op-permission-level=2`, and seeds `whitelist.json` with every account of the launcher. An offline
  server accepts any name; the whitelist is what keeps a stranger who reaches the port from joining
  under the operator's name. A world created before the first account is whitelisted before it
  opens.
- The README names the repository as the development home of Aether, captions the frame-rate capture
  with the conditions it was measured in, and states that the combat modules are disabled by
  default and can get an account banned where they are forbidden.
- `docs/FEATURES.md` gains an "On public servers" column for the Combat category.

### Added

- The optimisation pack is resolved as a Modrinth dependency graph before the first download: the
  required dependencies are read from the API, a dependency that pins a `version_id` is installed at
  that exact version with its own SHA-1, a project is downloaded once even when the pack lists it
  and another mod requires it (Cloth Config), optional/embedded/incompatible declarations are
  reported, two different pinned versions of one project are an explicit conflict, and a node that
  does not announce the pack's game version or Fabric loader stops the installation while nothing
  has been written.
- `aether-launcher verify` hashes the client, the libraries, the asset index, every asset and the
  jars of the pack, and names the files that are missing or altered; `verify --repair` re-downloads
  only those.
- `install.pending.json`: `install` and `performance install` journal their run, are reported by
  `info` (`pendingInstall`), resume by skipping the files whose digest already matches, and
  `install --discard` removes the journal with the leftover `.part` files.
- `node --test launcher/gui/src/ui/format.test.mjs` covers `formatBytes` and the slider percentage
  the interface used to prove only through the end-to-end scripts.
- `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md` and `THIRD-PARTY-NOTICES.md`.
- `.github/workflows/release.yml`: a `v*` tag builds the mod, the core and the shell, publishes the
  installers and the mod jar with a `SHA256SUMS` file.
- `.github/dependabot.yml` for Cargo, Gradle, npm and GitHub Actions.
- The CI checks `cargo fmt --check` on the core and the shell.

### Removed

- `MimiMax.exe` and `Aether-Launcher.exe` leave the Git tracking: the root binaries are built by
  `build.bat` and delivered by Releases, never by a commit.
- The local Fabric server installations under `tools/fabric-host-test/` and `tools/fabric-server-test/`
  leave the Git tracking: 153 MB of libraries, worlds and logs, including a
  `fabric-server-launcher.properties` that named an absolute path of one machine.

## [1.0.0] - 2026-11-21

First documented release, published as the NSIS installer and a portable executable.

### Added

- Fabric client: Click GUI, configuration screen, HUD editor, HUD elements, modules, `/aether`.
- Launcher: offline and Microsoft accounts, version and Fabric installation, servers page with a
  real login probe, local server host, optimisation pack and JVM presets, Identity Studio.
- Offline mode: deterministic offline UUID, a Mojang ownership check before joining a cracked
  server, and a warning in the interface.
- Microsoft session tokens in a DPAPI-protected vault (`accounts.credentials`) instead of
  `accounts.json`, with a migration on first start.
- Hardening of downloaded input: path validation, zip-slip rejection, deduplicated assets, symlink
  rejection, atomic writes, one installation and one game at a time.

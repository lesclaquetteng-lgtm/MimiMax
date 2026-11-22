# Contributing

Thanks for helping. This document describes the shortest path from a clone to a change that the CI
accepts.

## Prerequisites

| Part | Needs |
| --- | --- |
| `launcher/` (Rust core and Tauri shell) | a stable Rust toolchain with `rustfmt` and `clippy`; WebView2 on Windows to run the shell |
| `aether-client/` (Fabric mod) | JDK 21 to compile Minecraft 1.21.11, JDK 25 to run the current Gradle/Loom |
| `tools/` (checks and end-to-end scripts) | Node.js 22, but only for those scripts; the launcher itself has no npm dependency |

## Build and test

```powershell
# Launcher core
cd launcher
cargo test --release
cargo clippy --release --all-targets -- -D warnings
cargo fmt --check

# Native shell
cargo test --release --manifest-path desktop/Cargo.toml
cargo fmt --manifest-path desktop/Cargo.toml --check

# Fabric mod
cd ../aether-client
.\gradlew.bat build
```

The CI runs the Rust tests on Windows **and** Linux, checks the formatting, builds the shell with
the embedded core and mod, and runs the interface contracts (`tools/checks/*.mjs`). A pull request
that fails one of those jobs is not merged.

The behavioural end-to-end scripts are described in [`docs/VERIFICATION.md`](docs/VERIFICATION.md).
They start a window on a fake core and **delete accounts, mods and profiles**: every one of them
starts with `tools/e2e/guard.mjs`, which refuses to run unless `AETHER_HOME` is a home under
`tools/`. Never point them at your own installation.

## Style

- `cargo fmt` decides the Rust formatting; `clippy -D warnings` decides the rest.
- Every user-visible behaviour that can be tested without a display goes into a unit or integration
  test next to the code. A fix comes with the test that fails before it.
- Comments explain **why** a decision was made, in the language of the file around them; the code
  base is documented in English and the French reports under `docs/rapports/` are historical.
- No binary, no build output, no log and no secret is committed: `.gitignore` covers the artefacts
  of `build.bat`, the local server installations under `tools/`, and the homes of the scripts.
  Releases are produced by the `release` workflow, with `SHA256SUMS`.

## Pull requests

- Keep a change to one subject, and say which command proves it.
- Say what you did **not** verify (a screen you did not open, a platform you do not have).
- Report a security problem through [`SECURITY.md`](SECURITY.md), never in a pull request.

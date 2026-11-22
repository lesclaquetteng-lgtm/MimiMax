# Security policy

## Supported versions

Only the latest published release of the launcher and the mod matching it are supported. The
installers are built from this repository by the `release` workflow; a binary whose `SHA256SUMS`
entry does not match the attached file is not a release of this project.

## Reporting a vulnerability

**Do not open a public issue.** Use GitHub's private vulnerability reporting:

1. open the **Security** tab of this repository;
2. choose **Report a vulnerability**;
3. describe the problem, the version, the platform and the steps that reproduce it.

Please include, when they apply:

- the command or the screen concerned (`account`, `install`, `launch`, `servers probe`, `hosts`, the
  launcher interface, or a module of the mod);
- the log file written under `<home>/Aether/aether/logs/`, **with the session tokens removed**;
- what an attacker gains, not only what crashes.

Reports are handled on a best-effort basis; there is no paid support and no guaranteed delay. A
fix is published as a normal release, and the report is credited unless you ask otherwise.

## What is in scope

- the Rust launcher core (`launcher/src`);
- the Tauri shell and its interface (`launcher/desktop`, `launcher/gui`);
- the Fabric mod (`aether-client`);
- the defaults of the hosted server the launcher generates.

## What is out of scope

- vulnerabilities of Minecraft, Mojang, Microsoft or of a mod installed by the optimisation pack:
  report them to their author;
- a ban, a kick or a refusal by a third-party server, including the refusal of an offline name that
  a Mojang account already owns (that refusal is the server's rule, not a defect);
- the intended behaviour of the client described in the README: offline accounts, the offline
  server host, and the modules that a server may forbid. Using those on a server that forbids them
  is the player's responsibility.

## Secrets in this repository

Microsoft access and refresh tokens are never written to `accounts.json`: on Windows they live in
`accounts.credentials`, protected by DPAPI for the current Windows user, and the other platforms
refuse to persist them rather than writing them in clear text. If you find a token, a password or a
session identifier committed anywhere in this repository, treat it as a vulnerability and report it
privately.

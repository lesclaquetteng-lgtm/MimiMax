# Aether — The Fold

The launcher home is a threshold, not a dashboard.

## Interaction principles

- The home screen has one primary action: **ENTER**.
- The first screen shows only the active player, the current launcher state and the action to enter the game.
- Account switching is available in a collapsed player disclosure; full account management remains in the Accounts destination.
- Profile, Minecraft/Fabric version, memory, window size and repair controls stay in the collapsed Setup disclosure.
- Game output is kept under Diagnostics.
- Secondary destinations are grouped in the Explore menu instead of a permanent sidebar.

## Identity

The Fold is a custom three-plane geometric mark built directly into the launcher UI. Its three faces are deliberately asymmetric and share a cut-out that suggests a threshold. The same mark is used in the titlebar and as the main home-screen artwork.

The mark is inline SVG and CSS, so it does not depend on remote assets or network access. Motion is limited to the planes arriving into place and is disabled for users who request reduced motion.

## Preserved destinations

The existing pages remain available through Explore:

- Accounts
- Versions & profiles
- Mods
- Servers
- Performance
- Diagnostics
- Appearance

The Rust/Tauri backend and the existing launcher actions are unchanged. The UI retains the DOM hooks required by account, profile, installation, launch, progress, stop, and log handlers.

## Verification

Run:

\`\`\`sh
node --check launcher/gui/src/ui/app.js
node --check launcher/gui/src/ui/bridge.js
node tools/check-the-fold.mjs
node tools/check-theme-tokens.mjs
\`\`\`

The Theme tokens CI job runs these checks. The Fold contract script verifies the new navigation destinations and confirms the legacy launcher DOM hooks remain present.

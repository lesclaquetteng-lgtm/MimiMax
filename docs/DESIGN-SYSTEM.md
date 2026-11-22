# Aether Design System

The launcher and Fabric client share one visual vocabulary. The launcher defines web tokens in
`launcher/gui/src/ui/tokens.css`; the in-game client mirrors the same semantic values in
`aether-client/.../design/AetherDesignSystem.java`. CI checks all 48 material channels.

## Materials

| Material | Signature accent | Character |
| --- | --- | --- |
| Graphite | `#E7E9EC` | neutral, quiet, default |
| Oxide | `#D99A69` | warm industrial |
| Field | `#C0D18A` | organic and technical |
| Blueprint | `#83C5DF` | cool drafting-room blue |

Every material defines canvas, elevated canvas, surface, hover and active surfaces, two border
strengths, accent and four text-emphasis levels. Components consume semantic tokens rather than
inventing local colours for ordinary UI.

## Player preferences

The launcher and in-game client persist appearance independently because they are separate
processes and settings stores. In-game preferences live in `config/aether/config.json`:

- `colorway`: `graphite`, `oxide`, `field` or `blueprint`.
- `surfaceTreatment`: `contour`, `draft` or `quiet`.
- `customAccent` and `accentColor`: personal accent override. When disabled, the material
  signature accent is used.

Schema v1 and unversioned configurations migrate to schema v2. Existing custom colours are retained;
former built-in accent defaults inherit the selected material signature.

## Surface treatments

- **Contour**: restrained nested outlines behind the in-game workspace.
- **Draft**: a low-contrast technical grid.
- **Quiet**: no decorative linework.

Treatments are drawn behind the workspace and do not change hitboxes, module layout or saved HUD
positions.

## Accessibility and rendering

Text on accent-filled controls uses a calculated black or white foreground selected against WCAG
relative luminance. Status colours remain semantic and are not repurposed as material accents. The
active Java palette is cached by `RenderUtil`, keeping theme resolution out of the HUD drawing path.

## Verification

```sh
node tools/checks/check-theme-tokens.mjs
node tools/checks/check-identity-studio.mjs
node tools/checks/check-aether-design-system.mjs
cd aether-client && ./gradlew --no-daemon build
```

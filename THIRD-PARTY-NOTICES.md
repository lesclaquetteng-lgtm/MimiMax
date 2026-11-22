# Third-party notices

## Minecraft and Fabric

Minecraft is a trademark of Mojang Studios. This project is not affiliated with Mojang or Microsoft.
The launcher downloads the game, its libraries and its assets from Mojang's own servers, then adds
the Fabric loader and the Fabric API. The client mod is compiled against Minecraft with Fabric Loom.

## Optimisation pack

The **Performance** page downloads these mods **from Modrinth at installation time**; the repository
does not redistribute them and ships no copy of them. Each version is pinned by its SHA-1, and the
licence of each mod is the one its author declares on its project page — follow the link before
redistributing a pack built from them.

| Mod | Role | Source |
| --- | --- | --- |
| Sodium | Rendering pipeline | https://modrinth.com/mod/sodium |
| Lithium | Game-logic optimisation | https://modrinth.com/mod/lithium |
| FerriteCore | Memory usage | https://modrinth.com/mod/ferrite-core |
| ImmediatelyFast | Immediate-mode rendering | https://modrinth.com/mod/immediatelyfast |
| EntityCulling | Skips hidden entities | https://modrinth.com/mod/entityculling |
| MoreCulling | Culls more block faces | https://modrinth.com/mod/moreculling |
| Krypton | Network stack | https://modrinth.com/mod/krypton |
| BadOptimizations | Assorted micro-optimisations | https://modrinth.com/mod/badoptimizations |
| Dynamic FPS | Lowers the frame rate in the background | https://modrinth.com/mod/dynamic-fps |
| Clumps | Groups experience orbs | https://modrinth.com/mod/clumps |
| Cloth Config | Configuration library required by some of the above | https://modrinth.com/mod/cloth-config |
| Fabric API | Modding API required by the Aether client | https://modrinth.com/mod/fabric-api |

## Rust and Java dependencies

The launcher's Rust dependencies and their licences are listed by `cargo metadata` and by the
`Cargo.lock` files of `launcher/` and `launcher/desktop/`; the Fabric mod's dependencies are listed
by `aether-client/build.gradle`. Both are used under their own licences, unmodified except for the
bundling the build performs.

## Assets

The icons, the screenshots under `docs/images/` and the colourways of the Identity Studio are part of
this project and covered by its MIT licence, except the screenshots that show the Minecraft
interface, where Minecraft's own terms apply.

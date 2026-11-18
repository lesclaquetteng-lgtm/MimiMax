# Aether Client

The Fabric mod of the **Aether Client**. It adds the interface, the modules and the HUD of a
professional Minecraft client to the game.

![The click GUI](../docs/images/click-gui.png)

## Requirements

| | |
| --- | --- |
| Minecraft | 1.21.11 |
| Loader | Fabric 0.20.1 |
| Fabric API | 0.141.6+1.21.11 |
| Java | 21 |
| Mappings | the official Mojang mappings |

## Building

Gradle itself needs a **JDK 25** (Fabric Loom 1.19 requires it), the game is compiled and started
with **Java 21**. The toolchain is installed by `../tools/setup.ps1`, the Java runtime for the game
is downloaded automatically by Gradle.

```powershell
..\tools\gradle-9.8.2\bin\gradle.bat build
```

The result is `build/libs/aether-client-1.0.0.jar`, a mod that can be dropped into the `mods`
folder of any Fabric installation.

```powershell
..\tools\gradle-9.8.2\bin\gradle.bat runClient
```

## The features

| Category | Modules |
| --- | --- |
| HUD | FPS, Ping, CPS, Keystrokes, Armor, Potions, Coordinates, Server Brand, Held Item, Session Timer, Target HUD, Session |
| Render | Fullbright, FOV, No View Bobbing, Crosshair, Watermark |
| Combat | Auto Tool, Kill Aura, Auto Crystal, Anti Knockback |
| Movement | Auto Sprint, No Fall, Safe Walk, Auto Jump |
| Misc | Hide Scoreboard, Notifications |

The click GUI opens with `RIGHT SHIFT`, on the main menu as well as in game.

## The structure

```
src/main/java/dev/aether/client
├── AetherClient.java      the entry point
├── Aether.java            the singleton with the state
├── config/                the persistent configuration
├── module/                the module framework and the modules
├── setting/               the typed settings
├── hud/                   the HUD manager
├── gui/                   the click GUI, the configuration screen, the HUD editor
├── util/                  rendering, colours, animations, the click tracker, the session
└── mixin/                 the five places where the game has to be adapted
```

The details are in [`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md).

## The tests

```powershell
..\tools\gradle-9.8.2\bin\gradle.bat test
```

19 tests cover the offline identifier, the configuration, the helpers and the mixins. The mixin
tests read the class files with ASM and verify that every mixin exists, that it patches a class of
the game and that every injection point still exists with a matching callback type.

# Features

Every feature of the Aether client, what it does and how it is used.

## Interface

### Click GUI — `RIGHT SHIFT`

Opens on the main menu and in game. The left column switches between the categories, the right
panel lists the modules of the category.

| Action | Result |
| --- | --- |
| Left click on a module | Enables or disables it. |
| Right click on a module | Opens its settings. |
| Left click on a setting | Changes the value (boolean toggles, modes cycle, numbers step). |
| Right click on a setting | Changes the value backwards. |
| Click on the search field | Filters the modules of the category. |
| Scroll | Moves the list. |
| `ESC` | Closes the interface and saves the configuration. |

The switch of a module is animated, the interface uses the accent colour of the player.

### Configuration screen

Opens with `J` by default (`key.aether.config`, rebindable in the controls screen). It shows the
session of the player, the accent colour, the keybind of every module (click a keybind and press the
new one), a HUD editor and the actions to reset the modules or the settings.

### HUD editor

Opens with `H` by default (`key.aether.hud_editor`, rebindable in the controls screen) or from the
configuration screen. Every HUD element can be dragged around, and the positions are saved as soon
as they change.

### Main menu

The background of the panorama is replaced by the design of the client, the name of the client is
shown in the corner and the state of the session is displayed as a badge. Offline sessions hide
the realms button because it cannot be used without a premium account.

### Command

```
/aether
```

Prints the version of the client and the type of the session.

## HUD

| Element | Description | Default |
| --- | --- | --- |
| FPS | The frame rate, optionally with the maximum and the average. | on |
| FPS Graph | The frame rate of the last three seconds as green/orange/red bars, under the watermark. | off |
| Ping | The latency towards the server, coloured between green, yellow and red. | on |
| CPS | The clicks per second of the left mouse button. | on |
| Keystrokes | The keys that are currently pressed, including the mouse buttons. | off |
| Armor | The equipped armour and the off hand. | off |
| Potions | The active potion effects with their duration. | off |
| Coordinates | The position, the direction and the biome of the player. | off |
| Server Brand | The software the server runs on. | off |
| Held Item | The item in the main hand. | off |
| Session Timer | How long the current session lasts. | off |
| Target HUD | The entity the player is aiming at with its health. | off |
| Session | The name of the player and the type of the session. | off |

## Modules

### Render

| Module | Description | Settings |
| --- | --- | --- |
| Fullbright | Removes the darkness of the world. | Gamma |
| FOV | Changes the field of view. | Value (30 - 170) |
| No View Bobbing | Stops the camera from moving while walking. | – |
| Crosshair | Replaces the crosshair of the game. | Size, Thickness, Center dot |
| Watermark | Shows the name and the version of the client. | – |

### Combat

| Module | Description | Settings |
| --- | --- | --- |
| Auto Tool | Switches to the tool that fits the block that is mined. | Switch back |
| Kill Aura | Attacks the closest enemy while the attack key is held (`G`). | Range, Cooldown, Rotate, Only players, Indicator |
| Auto Crystal | Breaks and places end crystals (`V`). | Place, Break, Range, Place delay, Break delay |
| Anti Knockback | Reduces the knockback the player receives. | Horizontal, Vertical |

### Movement

| Module | Description | Settings |
| --- | --- | --- |
| Auto Sprint | Sprints automatically while walking forwards. | – |
| No Fall | Prevents the fall damage of the player. | Only void |
| Safe Walk | Keeps the player from walking off the edge. | Sprint |
| Auto Jump | Jumps automatically while walking. | – |

### Misc

| Module | Description | Settings |
| --- | --- | --- |
| Hide Scoreboard | Hides the scoreboard of the server. | – |
| Notifications | Shows the chat messages in the corner of the screen. | Game messages, Duration |

## Launcher

| Feature | Description |
| --- | --- |
| Offline accounts | A name is enough, the identifier is generated like Minecraft does it. |
| Microsoft accounts | The device code flow, the session is refreshed automatically. |
| Versions | Every released Minecraft version, the Fabric loaders are resolved per version. |
| Installation | Downloads the game, the libraries, the assets, the Fabric loader, the Fabric API and the Aether mod. |
| Identity Studio | A live workspace preview with four material colourways, custom accent, contour/drafting/plain surfaces, roomy/compact density and expanded/icon-rail navigation. All choices persist locally. |
| Runtime settings | Java path, memory allocation, garbage collector and Fabric API version. |
| Servers | Ping, offline-account probe, saved list, one-click join. |
| Hosting | Creates and runs a local Fabric server with `online-mode=false`, saves the world before stopping. |
| Performance | Installs and verifies the optimisation pack, writes the game options and the JVM flags. |
| Scriptable | The same features are available on the command line. |

### Servers page

| Control | Result |
| --- | --- |
| Name + Address, **Add server** | Saves the server and pings it at once. |
| **Test** on a row, or **Test address** | Pings the server **and** probes its login: `Offline accounts accepted`, `Premium only`, `Refused` or `Unreachable`, with the player counts, the latency and the version. |
| **Join** | Starts the game with `--quickPlayMultiplayer <address>`; the account is the selected one. |
| **Remove** | Drops the entry from the saved list. |
| **Create the server** | Installs Fabric once (shared) and prepares a world directory with `online-mode=false`, a random RCON password and the requested port and player limit. |
| **Start** / **Stop** | Starts the server and waits until it answers the ping, or stops it through RCON so the chunks are saved. |
| **Join** on a local server | Launches the game on `127.0.0.1:<port>`. |

### Performance page

| Control | Result |
| --- | --- |
| **Install the pack** | Resolves every mod on Modrinth for the selected game version, verifies the SHA-1, replaces the old file, then lists what is installed. |
| **Maximum FPS** | Writes the `max` options preset, after copying the original `options.txt` to `options.txt.aether-backup`. |
| **Balanced** | Same thing with a world that keeps its clouds and a render distance of 12. |
| **Restore** | Puts the backup back. |
| JVM preset | `balanced`, `performance` or `ultra`, applied on the next launch. |

The page also prints the wear of the pack (`11/11`), the folder of the mods and the preset that was
applied last.


### Safe recovery after repeated abnormal shutdowns

After two consecutive sessions that do not reach Fabric's normal stopping callback, Aether offers
a title-screen choice: start with Aether modules disabled for this session, or continue normally.
The notice explains that abnormal shutdowns are not proof that Aether caused a crash. The optional
`/aether safe-mode` command is also available in-game. Saved module preferences are preserved and
return on the next normal launch.

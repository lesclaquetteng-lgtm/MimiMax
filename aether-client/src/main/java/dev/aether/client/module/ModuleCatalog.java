package dev.aether.client.module;

import dev.aether.client.module.modules.CombatModules;
import dev.aether.client.module.modules.HudModules;
import dev.aether.client.module.modules.MiscModules;
import dev.aether.client.module.modules.MovementModules;
import dev.aether.client.module.modules.RenderModules;

import java.util.ArrayList;
import java.util.List;

/**
 * Every module the client offers, in display order.
 *
 * <p>The list used to live inside the entry point, where it could not be read without starting the
 * game. It deliberately does not carry keys: a module is registered under its own name, in lower
 * case, so writing the key a second time here would recreate the drift that made a mixin look for a
 * module that no longer existed. {@code ModuleCatalogTest} builds a manager from this list and asks
 * it for every key a mixin reads.</p>
 */
public final class ModuleCatalog {

	private ModuleCatalog() {
	}

	/**
	 * @return the modules of the client, in display order
	 */
	public static List<Module> create() {
		List<Module> modules = new ArrayList<>();

		// HUD
		modules.add(new HudModules.Fps());
		modules.add(new HudModules.FpsGraph());
		modules.add(new HudModules.Ping());
		modules.add(new HudModules.Cps());
		modules.add(new HudModules.Keystrokes());
		modules.add(new HudModules.Armor());
		modules.add(new HudModules.Potions());
		modules.add(new HudModules.Coordinates());
		modules.add(new HudModules.ServerBrand());
		modules.add(new HudModules.HeldItem());
		modules.add(new HudModules.SessionTimer());
		modules.add(new HudModules.TargetHud());
		modules.add(new HudModules.Session());
		modules.add(new HudModules.Speed());
		modules.add(new HudModules.ComboCounter());
		modules.add(new HudModules.ReachDisplay());
		modules.add(new HudModules.Direction());
		modules.add(new HudModules.Clock());
		modules.add(new HudModules.Memory());

		// Render
		modules.add(new RenderModules.Fullbright());
		modules.add(new RenderModules.Fov());
		modules.add(new RenderModules.NoViewBobbing());
		modules.add(new RenderModules.Crosshair());
		modules.add(new RenderModules.Watermark());
		modules.add(new RenderModules.Esp());
		modules.add(new RenderModules.HideFireOverlay());
		modules.add(new RenderModules.HideHurtOverlay());
		modules.add(new RenderModules.AntiBlind());
		modules.add(new RenderModules.Zoom());

		// Combat
		modules.add(new CombatModules.AutoTool());
		modules.add(new CombatModules.KillAura());
		modules.add(new CombatModules.AutoCrystal());
		modules.add(new CombatModules.AntiKnockback());

		// Movement
		modules.add(new MovementModules.AutoSprint());
		modules.add(new MovementModules.NoFall());
		modules.add(new MovementModules.SafeWalk());
		modules.add(new MovementModules.AutoJump());
		modules.add(new MovementModules.FastSneak());

		// Misc
		modules.add(new MiscModules.HideScoreboard());
		modules.add(new MiscModules.Notifications());
		modules.add(new MiscModules.AutoGG());
		modules.add(new MiscModules.AntiAFK());

		return modules;
	}
}

package dev.aether.client.module;

import net.minecraft.client.gui.GuiGraphics;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Holds every module of the client and dispatches the lifecycle callbacks.
 */
public final class ModuleManager {
	private final Map<String, Module> modules = new LinkedHashMap<>();

	/**
	 * Registers a module so that it shows up in the click GUI.
	 */
	public void register(Module module) {
		modules.put(module.getName().toLowerCase(java.util.Locale.ROOT), module);
	}

	/**
	 * @return the module with the given name, or null.
	 */
	public Module get(String name) {
		return modules.get(name.toLowerCase(java.util.Locale.ROOT));
	}

	/**
	 * @return every registered module.
	 */
	public List<Module> getAll() {
		return new ArrayList<>(modules.values());
	}

	/**
	 * @return every module of the given category.
	 */
	public List<Module> getByCategory(ModuleCategory category) {
		List<Module> result = new ArrayList<>();
		for (Module module : modules.values()) {
			if (module.getCategory() == category) {
				result.add(module);
			}
		}
		return result;
	}

	/**
	 * @return the modules grouped by their category, keeping the registration order.
	 */
	public Map<ModuleCategory, List<Module>> getGrouped() {
		Map<ModuleCategory, List<Module>> grouped = new LinkedHashMap<>();
		for (ModuleCategory category : ModuleCategory.values()) {
			grouped.put(category, new ArrayList<>());
		}
		for (Module module : modules.values()) {
			grouped.get(module.getCategory()).add(module);
		}
		return grouped;
	}

	/**
	 * Toggles the module with the given name.
	 * @return true when a module was found.
	 */
	public boolean toggle(String name) {
		Module module = get(name);
		if (module == null) {
			return false;
		}
		module.toggle();
		return true;
	}

	/**
	 * Calls {@link Module#onTick()} on every enabled module.
	 */
	public void tick() {
		for (Module module : modules.values()) {
			if (module.isEnabled()) {
				module.invokeSafely("onTick", module::onTick);
			}
		}
	}

	/**
	 * Calls {@link Module#onRender(GuiGraphics, float)} on every enabled module that is not a HUD
	 * element.
	 *
	 * <p>The HUD elements are drawn by the {@link dev.aether.client.hud.HudManager}, which knows
	 * their positions; drawing them here as well would render every element twice, once at the
	 * origin.</p>
	 */
	public void render(GuiGraphics graphics, float partialTick) {
		for (Module module : modules.values()) {
			if (module.isEnabled() && !(module instanceof HudElement)) {
				module.invokeSafely("onRender", () -> module.onRender(graphics, partialTick));
			}
		}
	}

	/**
	 * Advances the animations of every module.
	 */
	public void updateAnimations(long deltaMillis) {
		for (Module module : modules.values()) {
			module.updateAnimations(deltaMillis);
		}
	}

	/**
	 * Disables every module for the current runtime. Safe-mode callers deliberately avoid capturing
	 * this temporary state back into persisted preferences.
	 */
	public void disableAll() {
		for (Module module : modules.values()) {
			module.setEnabled(false);
		}
	}

	/**
	 * Restores every module to its default state.
	 */
	public void reset() {
		for (Module module : modules.values()) {
			module.reset();
		}
	}

}

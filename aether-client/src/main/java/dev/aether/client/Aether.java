package dev.aether.client;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.util.ClickTracker;

/**
 * The central point of the client.
 *
 * Everything that has to be reachable from everywhere - the configuration, the module manager and
 * the click tracker - lives here.
 */
public final class Aether {
	public static final String MOD_ID = "aether";
	public static final String NAME = "Aether Client";
	public static final String VERSION = "1.0.0";

	private static Aether instance;

	private final AetherConfig config;
	private final ModuleManager moduleManager;
	private final ClickTracker clickTracker;

	private Aether(AetherConfig config, ModuleManager moduleManager) {
		this.config = config;
		this.moduleManager = moduleManager;
		this.clickTracker = new ClickTracker();
	}

	/**
	 * Creates the singleton. It is called once while the client starts.
	 */
	public static Aether create(AetherConfig config, ModuleManager moduleManager) {
		instance = new Aether(config, moduleManager);
		return instance;
	}

	/**
	 * @return the singleton.
	 * @throws IllegalStateException when the client was not created yet.
	 */
	public static Aether get() {
		if (instance == null) {
			throw new IllegalStateException("The Aether client was not initialised yet.");
		}
		return instance;
	}

	/**
	 * @return the singleton, or null when the client was not created yet.
	 */
	public static Aether getOrNull() {
		return instance;
	}

	public AetherConfig getConfig() {
		return config;
	}

	public ModuleManager getModuleManager() {
		return moduleManager;
	}

	public ClickTracker getClickTracker() {
		return clickTracker;
	}

	/**
	 * Writes the current state of the client to the disk.
	 */
	public void save() {
		config.captureFrom(moduleManager);
		config.save();
	}
}

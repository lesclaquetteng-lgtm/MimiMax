package dev.aether.client.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.ColorSetting;
import dev.aether.client.setting.DoubleSetting;
import dev.aether.client.setting.IntegerSetting;
import dev.aether.client.setting.ModeSetting;
import dev.aether.client.setting.Setting;
import dev.aether.client.util.ColorUtil;
import net.fabricmc.loader.api.FabricLoader;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * The persistent configuration of the client.
 *
 * Everything the player changes is written to {@code config/aether/config.json} and restored the
 * next time the game starts.
 */
public final class AetherConfig {
	public static final String DEFAULT_ACCENT = "#E7E9EC";

	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	/** The accent colour used by the interface. */
	public String accentColor = DEFAULT_ACCENT;

	/** Whether the client shows a notification when it joins or leaves a world. */
	public boolean showNotifications = true;

	/** Whether the click GUI opens with the right shift key. */
	public boolean openClickGuiWithRightShift = true;

	/** The enabled state of every module, keyed by the lower case module name. */
	public Map<String, Boolean> modules = new LinkedHashMap<>();

	/** The settings of every module, keyed by the lower case module name. */
	public Map<String, Map<String, Object>> settings = new LinkedHashMap<>();

	/** The position of every HUD element, keyed by the lower case element name. */
	public Map<String, int[]> hudPositions = new LinkedHashMap<>();

	/**
	 * @return the path of the configuration file.
	 */
	public static Path getPath() {
		return FabricLoader.getInstance().getConfigDir().resolve("aether").resolve("config.json");
	}

	/**
	 * Reads the configuration from the disk. A missing or broken file simply produces the defaults.
	 */
	public static AetherConfig load() {
		return load(getPath());
	}

	/**
	 * Reads the configuration from the given file.
	 * @param path the file to read
	 */
	public static AetherConfig load(Path path) {
		if (Files.exists(path)) {
			try {
				String json = stripByteOrderMark(Files.readString(path, StandardCharsets.UTF_8));
				AetherConfig config = GSON.fromJson(json, AetherConfig.class);
				if (config != null) {
					config.sanitize();
					return config;
				}
			} catch (IOException | RuntimeException exception) {
				AetherClientLogger.warn("Could not read the configuration, using the defaults: " + exception.getMessage());
			}
		}
		AetherConfig config = new AetherConfig();
		config.sanitize();
		return config;
	}

	/**
	 * Removes the marker an editor such as Notepad writes in front of a UTF-8 file.
	 *
	 * Gson refuses a document that starts with it, and the whole configuration would silently fall
	 * back to the defaults: an accent colour or a HUD layout typed by hand would be lost.
	 */
	private static String stripByteOrderMark(String json) {
		if (!json.isEmpty() && json.charAt(0) == '\uFEFF') {
			return json.substring(1);
		}
		return json;
	}

	/**
	 * The accent a build shipped before the graphite identity. A configuration that still carries
	 * one of them never expressed a choice, so it must not keep the old violet on the interface
	 * while the launcher wears the new palette.
	 */
	private static final java.util.Set<String> LEGACY_ACCENTS =
			java.util.Set.of("#6C5CE7", "#7C5CFC", "#3B82F6");

	private void sanitize() {
		if (accentColor == null || accentColor.isBlank()
				|| LEGACY_ACCENTS.contains(accentColor.trim().toUpperCase(java.util.Locale.ROOT))) {
			accentColor = DEFAULT_ACCENT;
		}
		if (modules == null) {
			modules = new LinkedHashMap<>();
		}
		if (settings == null) {
			settings = new LinkedHashMap<>();
		}
		if (hudPositions == null) {
			hudPositions = new LinkedHashMap<>();
		}
	}

	/**
	 * Writes the configuration to the disk.
	 */
	public void save() {
		save(getPath());
	}

	/**
	 * Writes the configuration to the given file.
	 * @param path the file to write
	 */
	public void save(Path path) {
		try {
			Files.createDirectories(path.getParent());
			Files.writeString(path, GSON.toJson(this), StandardCharsets.UTF_8);
		} catch (IOException exception) {
			AetherClientLogger.warn("Could not write the configuration: " + exception.getMessage());
		}
	}

	/**
	 * Applies the stored state to the modules.
	 */
	public void applyTo(ModuleManager manager) {
		for (Module module : manager.getAll()) {
			String key = module.getName().toLowerCase(java.util.Locale.ROOT);

			// The settings come first: the modules that write into the options read them in
			// onEnable (Fullbright reads Gamma, FOV reads Value), so enabling first applied the
			// default and the saved value only reached the game on the next tick, or never for a
			// module that has no tick.
			Map<String, Object> stored = settings.get(key);
			if (stored != null) {
				for (Setting setting : module.getSettings()) {
					Object value = stored.get(setting.getName());
					if (value != null) {
						setting.setValue(value);
					}
				}
			}

			Boolean enabled = modules.get(key);
			if (enabled != null) {
				module.setEnabled(enabled);
			}
		}
	}

	/**
	 * Copies the current state of the modules into the configuration.
	 */
	public void captureFrom(ModuleManager manager) {
		modules.clear();
		settings.clear();
		for (Module module : manager.getAll()) {
			String key = module.getName().toLowerCase(java.util.Locale.ROOT);
			modules.put(key, module.isEnabled());

			Map<String, Object> moduleSettings = new LinkedHashMap<>();
			for (Setting setting : module.getSettings()) {
				moduleSettings.put(setting.getName(), setting.getValue());
			}
			if (!moduleSettings.isEmpty()) {
				settings.put(key, moduleSettings);
			}
		}
	}

	/**
	 * @return the accent colour as a packed colour.
	 */
	public int getAccentColor() {
		return ColorUtil.fromHex(accentColor, ColorUtil.fromHex(DEFAULT_ACCENT, 0xFF6C5CE7));
	}

	/**
	 * Restores every setting to its default value.
	 */
	public void resetSettings(ModuleManager manager) {
		for (Module module : manager.getAll()) {
			for (Setting setting : module.getSettings()) {
				setting.reset();
			}
		}
	}

}

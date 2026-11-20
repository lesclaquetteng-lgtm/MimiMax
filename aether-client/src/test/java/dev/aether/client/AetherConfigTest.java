package dev.aether.client;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.IntegerSetting;
import dev.aether.client.setting.ModeSetting;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Tests the configuration and the module registry of the client.
 */
class AetherConfigTest {

	/**
	 * A module that only exists for the tests.
	 */
	private static final class TestModule extends Module {
		private final BooleanSetting flag = register(new BooleanSetting("Flag", "A test flag", true));
		private final IntegerSetting amount = register(new IntegerSetting("Amount", "A test amount", 5, 0, 10));
		private final ModeSetting mode = register(new ModeSetting("Mode", "A test mode", "A", "A", "B"));

		private TestModule(String name, ModuleCategory category, boolean enabled) {
			super(name, "A module used by the tests", category, enabled);
		}
	}

	/**
	 * A module that records the value of its setting when it is enabled, the way Fullbright and FOV
	 * read theirs in {@code onEnable}.
	 */
	private static final class RecordingModule extends Module {
		private final IntegerSetting amount = register(new IntegerSetting("Amount", "A test amount", 5, 0, 10));
		private int valueOnEnable = -1;

		private RecordingModule() {
			super("Recording", "A module used by the tests", ModuleCategory.MISC, false);
		}

		@Override
		public void onEnable() {
			valueOnEnable = amount.get();
		}
	}

	@Test
	@DisplayName("the settings of a module are applied before the module is enabled")
	void settingsAreAppliedBeforeTheModuleIsEnabled() {
		ModuleManager manager = new ModuleManager();
		RecordingModule module = new RecordingModule();
		manager.register(module);

		AetherConfig config = new AetherConfig();
		Map<String, Object> stored = new LinkedHashMap<>();
		stored.put("Amount", 9);
		config.settings.put("recording", stored);
		config.modules.put("recording", true);

		config.applyTo(manager);

		assertTrue(module.isEnabled());
		assertEquals(9, module.valueOnEnable, "onEnable has to read the saved value, not the default");
	}

	@Test
	@DisplayName("safe mode applies module settings without restoring enabled flags")
	void safeModeKeepsModulesDisabledButPreservesSavedSettings() {
		ModuleManager manager = new ModuleManager();
		TestModule module = new TestModule("Test", ModuleCategory.MISC, true);
		manager.register(module);

		AetherConfig config = new AetherConfig();
		config.modules.put("test", true);
		Map<String, Object> stored = new LinkedHashMap<>();
		stored.put("Amount", 9);
		config.settings.put("test", stored);

		config.applyTo(manager, false);

		assertFalse(module.isEnabled(), "safe mode must not restore the persisted enabled state");
		assertEquals(9, ((IntegerSetting) module.getSetting("Amount")).get(),
				"safe mode still loads user settings");
	}

	@Test
	@DisplayName("the configuration survives a round trip through the disk")
	void configurationRoundTrip(@TempDir Path directory) {
		Path file = directory.resolve("config.json");

		AetherConfig config = new AetherConfig();
		config.accentColor = "#2ECC8F";
		config.showNotifications = false;
		config.modules.put("test", true);
		Map<String, Object> settings = new LinkedHashMap<>();
		settings.put("Flag", false);
		settings.put("Amount", 9);
		settings.put("Mode", "B");
		config.settings.put("test", settings);
		config.hudPositions.put("fps", new int[] { 10, 20 });
		config.save(file);

		AetherConfig loaded = AetherConfig.load(file);
		assertEquals("#2ECC8F", loaded.accentColor);
		assertFalse(loaded.showNotifications);
		assertTrue(loaded.modules.get("test"));
		assertEquals(10, loaded.hudPositions.get("fps")[0]);
		assertEquals(20, loaded.hudPositions.get("fps")[1]);
	}

	@Test
	@DisplayName("a missing configuration file produces the defaults")
	void missingConfiguration(@TempDir Path directory) {
		AetherConfig config = AetherConfig.load(directory.resolve("missing.json"));
		assertEquals(AetherConfig.DEFAULT_ACCENT, config.accentColor);
		assertTrue(config.modules.isEmpty());
	}

	@Test
	@DisplayName("a configuration saved by Notepad keeps its values")
	void configurationWithAByteOrderMark(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		String json = "{\"accentColor\":\"#2ECC8F\",\"modules\":{\"fps graph\":true}}";

		// Notepad writes the UTF-8 marker in front of the document; Gson refuses it and the whole
		// file would fall back to the defaults.
		java.nio.file.Files.writeString(
			file,
			"\uFEFF" + json,
			java.nio.charset.StandardCharsets.UTF_8
		);

		AetherConfig config = AetherConfig.load(file);
		assertEquals("#2ECC8F", config.accentColor);
		assertTrue(config.modules.get("fps graph"));
	}

	@Test
	@DisplayName("the state of the modules is applied and captured")
	void moduleStateRoundTrip() {
		ModuleManager manager = new ModuleManager();
		TestModule module = new TestModule("Test", ModuleCategory.MISC, false);
		manager.register(module);

		AetherConfig config = new AetherConfig();
		config.modules.put("test", true);
		((BooleanSetting) module.getSetting("Flag")).setValue(false);
		((IntegerSetting) module.getSetting("Amount")).setValue(7);
		((ModeSetting) module.getSetting("Mode")).setValue("B");

		config.applyTo(manager);
		assertTrue(module.isEnabled(), "the module was enabled by the configuration");
		assertFalse(((BooleanSetting) module.getSetting("Flag")).isEnabled());
		assertEquals(7, ((IntegerSetting) module.getSetting("Amount")).get());
		assertEquals("B", ((ModeSetting) module.getSetting("Mode")).get());

		config.captureFrom(manager);
		assertTrue(config.modules.get("test"));
		assertEquals(false, config.settings.get("test").get("Flag"));
		assertEquals(7, config.settings.get("test").get("Amount"));
		assertEquals("B", config.settings.get("test").get("Mode"));
	}

	@Test
	@DisplayName("modules are grouped by their category")
	void modulesAreGroupedByCategory() {
		ModuleManager manager = new ModuleManager();
		manager.register(new TestModule("First", ModuleCategory.HUD, false));
		manager.register(new TestModule("Second", ModuleCategory.HUD, false));
		manager.register(new TestModule("Third", ModuleCategory.RENDER, false));

		assertEquals(3, manager.getAll().size());
		assertEquals(2, manager.getByCategory(ModuleCategory.HUD).size());
		assertEquals(1, manager.getByCategory(ModuleCategory.RENDER).size());
		assertEquals(2, manager.getGrouped().get(ModuleCategory.HUD).size());
		assertNotNull(manager.get("first"));
		assertNotNull(manager.get("FIRST"));
	}

	@Test
	@DisplayName("toggling a module runs the lifecycle callbacks")
	void togglingRunsTheLifecycle() {
		ModuleManager manager = new ModuleManager();
		TestModule module = new TestModule("Test", ModuleCategory.MISC, false);
		manager.register(module);

		assertTrue(manager.toggle("Test"));
		assertTrue(module.isEnabled());

		assertTrue(manager.toggle("Test"));
		assertFalse(module.isEnabled());

		assertFalse(manager.toggle("Unknown"));
	}
}

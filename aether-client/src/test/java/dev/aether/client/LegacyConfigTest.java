package dev.aether.client;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.hud.HudManager;
import dev.aether.client.module.HudElement;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.setting.BooleanSetting;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * A configuration written by an older build has to keep loading.
 *
 * <p>The categories {@code Player} and {@code Interface} were removed from the interface because
 * they never held a module. A file that mentions them - or a module that does not exist anymore -
 * must still be read, must still apply the values it carries for the modules that do exist, and
 * must not make the client fail to start.</p>
 */
class LegacyConfigTest {

	/** A HUD element with a name the configuration can reference. */
	private static final class TestHud extends HudElement {
		private final BooleanSetting flag = register(new BooleanSetting("Flag", "A test flag", true));

		private TestHud(String name, boolean enabled) {
			super(name, "A HUD element used by the tests", ModuleCategory.HUD, enabled);
		}

		@Override
		public void render(net.minecraft.client.gui.GuiGraphics graphics, float partialTick) {
			// nothing to draw in a test
		}

		@Override
		public int measureWidth() {
			return 40;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/** The configuration a build with the Player and Interface tabs could have written. */
	private static final String LEGACY = """
			{
			  "accentColor": "#2ECC8F",
			  "showNotifications": false,
			  "openClickGuiWithRightShift": true,
			  "categories": ["Hud", "Player", "Interface"],
			  "modules": {
			    "fps": false,
			    "keystrokes": true,
			    "player info": true,
			    "interface tweaks": true
			  },
			  "settings": {
			    "fps": { "Max": true, "Average": false },
			    "keystrokes": { "Layout": "Compact" },
			    "player info": { "Radius": 4 }
			  },
			  "hudPositions": {
			    "fps": [12, 34],
			    "keystrokes": [5],
			    "ghost element": [1, 2]
			  }
			}
			""";

	@Test
	@DisplayName("an old configuration with unknown entries still loads")
	void anOldConfigurationWithUnknownEntriesStillLoads(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		Files.writeString(file, LEGACY, StandardCharsets.UTF_8);

		AetherConfig config = assertDoesNotThrow(() -> AetherConfig.load(file));

		assertEquals("#2ECC8F", config.accentColor, "the accent colour was lost");
		assertFalse(config.showNotifications);
		assertNotNull(config.modules);
		assertNotNull(config.settings);
		assertNotNull(config.hudPositions);
	}

	@Test
	@DisplayName("the values of the modules that still exist are applied")
	void theValuesOfTheExistingModulesAreApplied(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		Files.writeString(file, LEGACY, StandardCharsets.UTF_8);

		ModuleManager manager = new ModuleManager();
		TestHud fps = new TestHud("FPS", true);
		TestHud keystrokes = new TestHud("Keystrokes", false);
		manager.register(fps);
		manager.register(keystrokes);

		AetherConfig config = AetherConfig.load(file);
		assertDoesNotThrow(() -> config.applyTo(manager));

		assertFalse(fps.isEnabled(), "the stored state of FPS was not applied");
		assertTrue(keystrokes.isEnabled(), "the stored state of Keystrokes was not applied");
		assertTrue(((BooleanSetting) fps.getSetting("Flag")).isEnabled(), "the default of a missing setting was lost");
	}

	@Test
	@DisplayName("a removed module and its settings are ignored, not fatal")
	void aRemovedModuleIsIgnored(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		Files.writeString(file, LEGACY, StandardCharsets.UTF_8);

		ModuleManager manager = new ModuleManager();
		TestHud fps = new TestHud("FPS", true);
		manager.register(fps);

		AetherConfig config = AetherConfig.load(file);
		config.applyTo(manager);

		assertNull(manager.get("player info"), "a module that does not exist was invented");
		assertEquals(1, manager.getAll().size());
		assertFalse(fps.isEnabled());
	}

	@Test
	@DisplayName("the positions of the known elements are restored, the others are skipped")
	void thePositionsOfTheKnownElementsAreRestored(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		Files.writeString(file, LEGACY, StandardCharsets.UTF_8);

		ModuleManager manager = new ModuleManager();
		TestHud fps = new TestHud("FPS", true);
		TestHud keystrokes = new TestHud("Keystrokes", true);
		manager.register(fps);
		manager.register(keystrokes);

		HudManager hud = new HudManager();
		hud.register(fps);
		hud.register(keystrokes);

		AetherConfig config = AetherConfig.load(file);
		assertDoesNotThrow(() -> hud.applySavedPositions(config));

		assertEquals(12, fps.getX());
		assertEquals(34, fps.getY());
		// A position with a single number cannot be used, the default stays.
		assertEquals(0, keystrokes.getX());
		assertEquals(0, keystrokes.getY());
	}

	@Test
	@DisplayName("an old configuration is written back and read again")
	void anOldConfigurationSurvivesASave(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		Files.writeString(file, LEGACY, StandardCharsets.UTF_8);

		AetherConfig config = AetherConfig.load(file);

		ModuleManager manager = new ModuleManager();
		TestHud fps = new TestHud("FPS", true);
		manager.register(fps);

		config.applyTo(manager);
		config.captureFrom(manager);
		assertDoesNotThrow(() -> config.save(file));

		AetherConfig reloaded = AetherConfig.load(file);
		assertEquals("#2ECC8F", reloaded.accentColor);
		assertEquals(Boolean.FALSE, reloaded.modules.get("fps"));
		assertNull(reloaded.modules.get("player info"), "an entry of a module that does not exist was kept");
	}
}

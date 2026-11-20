package dev.aether.client.module;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;

/**
 * The real module list, registered the way the client registers it, checked without the game.
 *
 * <p>A mixin asks the manager for a module by key. When the key and the registration drifted apart,
 * the mixin found nothing, returned early and the game stayed perfectly silent: the hook was dead and
 * nothing failed. These assertions go through the same manager the mixins use, so a rename in either
 * place is a failure instead of a silence.</p>
 */
class ModuleCatalogTest {

	/** Registers the real catalogue, the way {@code AetherClient} does at startup. */
	private static ModuleManager populated() {
		ModuleManager manager = new ModuleManager();
		for (Module module : ModuleCatalog.create()) {
			manager.register(module);
		}
		return manager;
	}

	@Test
	@DisplayName("every key a mixin reads resolves to a module")
	void everyMixinKeyIsRegistered() {
		ModuleManager manager = populated();

		assertFalse(ModuleKeys.READ_BY_MIXINS.isEmpty(), "no key is declared");
		for (String key : ModuleKeys.READ_BY_MIXINS) {
			assertNotNull(manager.get(key), String.format(
					"a mixin reads the module \"%s\" but the manager has nothing under that key", key));
		}
	}

	@Test
	@DisplayName("no module is lost to a duplicate name")
	void noModuleIsLostToADuplicateName() {
		List<Module> catalog = ModuleCatalog.create();

		assertEquals(catalog.size(), populated().getAll().size(), String.format(
				"two modules share a key, so the manager keeps %d of the %d modules",
				populated().getAll().size(), catalog.size()));
	}

	@Test
	@DisplayName("the keys of the mixins are the lower case names of the modules they read")
	void mixinKeysFollowTheModuleNames() {
		ModuleManager manager = populated();

		for (String key : ModuleKeys.READ_BY_MIXINS) {
			Module module = manager.get(key);
			assertNotNull(module, key);
			assertEquals(key, module.getName().toLowerCase(java.util.Locale.ROOT), String.format(
					"the module \"%s\" is reachable as \"%s\" but not by its own name", module.getName(), key));
		}
	}
}

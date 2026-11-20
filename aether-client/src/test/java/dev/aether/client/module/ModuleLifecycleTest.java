package dev.aether.client.module;

import dev.aether.client.setting.IntegerSetting;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

/**
 * What {@code reset()} does to a module, checked without the game.
 *
 * <p>Reset used to write the flag directly, so a module that had changed an option of the game
 * (gamma, field of view, view bobbing, auto jump) was switched off without ever restoring it: the
 * option kept the value the module wanted and the only way back was to enable and disable the
 * module by hand.</p>
 */
class ModuleLifecycleTest {

	private static final class StubModule extends Module {
		private final IntegerSetting value = register(new IntegerSetting("Value", "A value", 5, 0, 10));
		private int enables;
		private int disables;

		private StubModule(boolean defaultEnabled) {
			super("Stub", "A module used by the tests", ModuleCategory.MISC, defaultEnabled);
		}

		@Override
		public void onEnable() {
			enables++;
		}

		@Override
		public void onDisable() {
			disables++;
		}
	}

	@Test
	@DisplayName("resetting a module the player enabled runs onDisable")
	void resetRunsTheDisableCallback() {
		StubModule module = new StubModule(false);
		module.setEnabled(true);

		module.reset();

		assertFalse(module.isEnabled());
		assertEquals(1, module.disables, "the options the module wrote have to be restored");
		assertEquals(0.0f, module.getToggleProgress(), 0.0f, "the switch snaps back, it does not glide");
	}

	@Test
	@DisplayName("resetting a module restores its settings as well")
	void resetRestoresTheSettings() {
		StubModule module = new StubModule(false);
		module.value.setValue(9);
		module.setEnabled(true);

		module.reset();

		assertEquals(5, module.value.get());
		assertEquals(1, module.disables);
	}

	@Test
	@DisplayName("resetting a module that was never touched runs no callback")
	void resetOfAnUntouchedModuleRunsNothing() {
		StubModule module = new StubModule(false);

		module.reset();

		assertFalse(module.isEnabled());
		assertEquals(0, module.enables);
		assertEquals(0, module.disables);
	}
}

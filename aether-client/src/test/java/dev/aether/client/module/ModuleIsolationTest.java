package dev.aether.client.module;

import dev.aether.client.hud.HudManager;
import net.minecraft.client.gui.GuiGraphics;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Regression tests for module callback isolation and lifecycle recovery. */
class ModuleIsolationTest {
	private static final class FaultyTickModule extends Module {
		private int ticks;
		private int disables;

		private FaultyTickModule() {
			super("Faulty tick", "Throws from onTick", ModuleCategory.MISC, true);
		}

		@Override
		public void onTick() {
			ticks++;
			throw new IllegalStateException("simulated tick failure");
		}

		@Override
		public void onDisable() {
			disables++;
		}
	}

	private static final class HealthyTickModule extends Module {
		private int ticks;

		private HealthyTickModule() {
			super("Healthy tick", "Keeps ticking", ModuleCategory.MISC, true);
		}

		@Override
		public void onTick() {
			ticks++;
		}
	}

	private static final class FaultyRenderModule extends Module {
		private int renders;

		private FaultyRenderModule() {
			super("Faulty render", "Throws from onRender", ModuleCategory.RENDER, true);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			renders++;
			throw new IllegalArgumentException("simulated render failure");
		}
	}

	private static final class HealthyRenderModule extends Module {
		private int renders;

		private HealthyRenderModule() {
			super("Healthy render", "Keeps rendering", ModuleCategory.RENDER, true);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			renders++;
		}
	}

	private static final class FaultyHud extends HudElement {
		private int renders;

		private FaultyHud(String name) {
			super(name, "A HUD module used by the test", ModuleCategory.HUD, true);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			renders++;
			throw new IllegalStateException("simulated HUD render failure");
		}

		@Override
		public int measureWidth() {
			return 20;
		}

		@Override
		public int measureHeight() {
			return 10;
		}
	}

	private static final class HealthyHud extends HudElement {
		private int renders;

		private HealthyHud() {
			super("Healthy HUD", "A healthy HUD module", ModuleCategory.HUD, true);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			renders++;
		}

		@Override
		public int measureWidth() {
			return 20;
		}

		@Override
		public int measureHeight() {
			return 10;
		}
	}

	private static final class BrokenEnableModule extends Module {
		private int cleanupCalls;

		private BrokenEnableModule() {
			super("Broken enable", "Fails during enable", ModuleCategory.MISC, false);
		}

		@Override
		public void onEnable() {
			throw new IllegalStateException("simulated enable failure");
		}

		@Override
		public void onDisable() {
			cleanupCalls++;
		}
	}

	@Test
	@DisplayName("a repeatedly failing tick callback is isolated and circuit-broken")
	void aFailingTickDoesNotBlockHealthyModules() {
		ModuleManager manager = new ModuleManager();
		FaultyTickModule faulty = new FaultyTickModule();
		HealthyTickModule healthy = new HealthyTickModule();
		manager.register(faulty);
		manager.register(healthy);

		manager.tick();
		manager.tick();
		manager.tick();
		manager.tick();

		assertEquals(3, faulty.ticks);
		assertFalse(faulty.isEnabled(), "the circuit breaker disables a module after repeated failures");
		assertEquals(1, faulty.disables, "disable cleanup runs exactly once");
		assertEquals(4, healthy.ticks, "a failing neighbour must not stop healthy modules");
	}

	@Test
	@DisplayName("render failures do not abort the render pass")
	void aFailingRenderDoesNotBlockHealthyModules() {
		ModuleManager manager = new ModuleManager();
		FaultyRenderModule faulty = new FaultyRenderModule();
		HealthyRenderModule healthy = new HealthyRenderModule();
		manager.register(faulty);
		manager.register(healthy);

		manager.render(null, 0.0f);
		manager.render(null, 0.0f);
		manager.render(null, 0.0f);
		manager.render(null, 0.0f);

		assertEquals(3, faulty.renders);
		assertFalse(faulty.isEnabled());
		assertEquals(4, healthy.renders);
	}

	@Test
	@DisplayName("HUD element failures are isolated from other HUD elements")
	void aFailingHudElementDoesNotBlockHealthyElements() {
		HudManager manager = new HudManager();
		FaultyHud faulty = new FaultyHud("Faulty HUD");
		HealthyHud healthy = new HealthyHud();
		manager.register(faulty);
		manager.register(healthy);

		manager.render(null, 0.0f);
		manager.render(null, 0.0f);
		manager.render(null, 0.0f);
		manager.render(null, 0.0f);

		assertEquals(3, faulty.renders);
		assertFalse(faulty.isEnabled());
		assertEquals(4, healthy.renders);
	}

	@Test
	@DisplayName("a failed enable rolls the enabled state back and attempts cleanup")
	void failedEnableLeavesTheModuleDisabled() {
		BrokenEnableModule module = new BrokenEnableModule();

		module.setEnabled(true);

		assertFalse(module.isEnabled());
		assertEquals(1, module.cleanupCalls);
		assertEquals(0.0f, module.getToggleProgress(), 0.0f);
	}

	@Test
	@DisplayName("fatal errors are not swallowed by the callback isolation boundary")
	void fatalErrorsAreNotCaught() {
		HealthyTickModule module = new HealthyTickModule();
		assertThrows(AssertionError.class,
				() -> module.invokeSafely("onTick", () -> { throw new AssertionError("fatal"); }));
	}
}

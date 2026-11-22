package dev.aether.client;

import dev.aether.client.hud.HudManager;
import dev.aether.client.module.HudElement;
import dev.aether.client.module.ModuleCategory;
import net.minecraft.client.gui.GuiGraphics;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertFalse;

class HudManagerTest {

	@Test
	void repeatedMeasurementFailuresAreContainedAndDisableTheBrokenElement() {
		HudManager manager = new HudManager();
		HudElement broken = new HudElement("Broken", "Test element", ModuleCategory.HUD, true) {
			@Override
			public int measureWidth() {
				throw new IllegalStateException("measurement failed");
			}

			@Override
			public int measureHeight() {
				return 10;
			}

			@Override
			public void onRender(GuiGraphics graphics, float partialTick) {
				// Frames between size measurements should keep rendering; the next scheduled
				// measurement retries and the circuit breaker eventually disables the element.
			}
		};
		manager.register(broken);

		assertDoesNotThrow(() -> {
			for (int frame = 0; frame <= 10; frame++) {
				manager.render(null, 0.0f);
			}
		});
		assertFalse(broken.isEnabled(), "the render circuit breaker should disable a repeatedly failing HUD element");
	}
}

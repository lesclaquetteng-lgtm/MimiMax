package dev.aether.client.module;

import dev.aether.client.module.modules.CombatModules;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The anti knockback module, driven the way the mixin drives it.
 *
 * <p>This is the test that would fail if the module went back to being decorative: it calls
 * {@code reduceKnockback} and {@code reduceAddedKnockback} with the values vanilla writes down for
 * a sword hit and for an explosion, and expects the settings to come back applied.</p>
 */
class AntiKnockbackTest {

	/** Vanilla knockback of a sword hit: pushed on x, up on y, back on z. */
	private static final double[] VANILLA = {0.4, 0.4, -0.4};

	private static CombatModules.AntiKnockback module() {
		return new CombatModules.AntiKnockback();
	}

	@Test
	@DisplayName("the module is off by default and gives the delta back untouched")
	void disabledLeavesTheDeltaAlone() {
		CombatModules.AntiKnockback module = module();

		assertFalse(module.isEnabled());
		assertFalse(module.isActive());
		assertArrayEquals(VANILLA, module.reduceKnockback(VANILLA[0], VANILLA[1], VANILLA[2]), 0.0);
	}

	@Test
	@DisplayName("enabled with the default settings, the horizontal knockback is cancelled")
	void defaultsCancelHorizontal() {
		CombatModules.AntiKnockback module = module();
		module.setEnabled(true);

		assertTrue(module.isActive());
		assertEquals(0.0, module.getHorizontalFactor(), 0.0);
		assertEquals(1.0, module.getVerticalFactor(), 0.0);
		assertArrayEquals(new double[] {0.0, 0.4, 0.0},
				module.reduceKnockback(VANILLA[0], VANILLA[1], VANILLA[2]), 1e-9);
	}

	@Test
	@DisplayName("the settings of the GUI drive the reduction")
	void settingsDriveTheReduction() {
		CombatModules.AntiKnockback module = module();
		module.setEnabled(true);
		module.getSetting("Horizontal").setValue(0.25);
		module.getSetting("Vertical").setValue(0.5);

		assertEquals(0.25, module.getHorizontalFactor(), 1e-9);
		assertEquals(0.5, module.getVerticalFactor(), 1e-9);
		assertArrayEquals(new double[] {0.1, 0.2, -0.1},
				module.reduceKnockback(VANILLA[0], VANILLA[1], VANILLA[2]), 1e-9);
	}

	@Test
	@DisplayName("turning the module off restores the vanilla knockback immediately")
	void disablingRestoresVanilla() {
		CombatModules.AntiKnockback module = module();
		module.setEnabled(true);
		module.setEnabled(false);

		assertFalse(module.isActive());
		assertArrayEquals(VANILLA, module.reduceKnockback(VANILLA[0], VANILLA[1], VANILLA[2]), 0.0);
	}

	@Test
	@DisplayName("an explosion push is reduced with the default settings")
	void explosionPushIsReduced() {
		CombatModules.AntiKnockback module = module();
		module.setEnabled(true);

		// The player was walking and falling: only the push of the packet may change.
		double[] reduced = module.reduceAddedKnockback(0.6, 0.5, -0.2, 0.1, -0.08, 0.05);

		assertArrayEquals(new double[] {-0.5, -0.08, 0.25}, reduced, 1e-9);
	}

	@Test
	@DisplayName("a disabled module keeps the explosion push")
	void disabledKeepsTheExplosionPush() {
		CombatModules.AntiKnockback module = module();

		double[] reduced = module.reduceAddedKnockback(0.6, 0.5, -0.2, 0.1, -0.08, 0.05);

		assertArrayEquals(new double[] {0.1, -0.08, 0.05}, reduced, 0.0);
	}
}

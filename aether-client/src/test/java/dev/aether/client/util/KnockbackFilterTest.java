package dev.aether.client.util;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The conditions that let the anti knockback module act, checked without the game.
 *
 * <p>Each of these decisions used to be an inline {@code return} of {@code
 * ClientPacketListenerMixin}. Getting one wrong is not a cosmetic bug: an id comparison that is too
 * loose reduces the velocity of every entity on screen, and a missing "push present" check would
 * make the explosion hook subtract a vector that was never added.</p>
 */
class KnockbackFilterTest {

	private static final int PLAYER = 42;
	private static final int OTHER_ENTITY = 43;

	@Test
	@DisplayName("a velocity packet for the local player is corrected while the module is on")
	void motionOfThePlayerIsCorrected() {
		assertTrue(KnockbackFilter.reducesMotion(true, PLAYER, PLAYER));
	}

	@Test
	@DisplayName("a velocity packet for another entity is never touched")
	void motionOfAnotherEntityIsLeftAlone() {
		assertFalse(KnockbackFilter.reducesMotion(true, OTHER_ENTITY, PLAYER));
	}

	@Test
	@DisplayName("a disabled module leaves every velocity packet alone, including the player's")
	void disabledModuleLeavesTheMotionAlone() {
		assertFalse(KnockbackFilter.reducesMotion(false, PLAYER, PLAYER));
		assertFalse(KnockbackFilter.reducesMotion(false, OTHER_ENTITY, PLAYER));
	}

	@Test
	@DisplayName("an explosion carrying a push is corrected while the module is on")
	void explosionWithAPushIsCorrected() {
		assertTrue(KnockbackFilter.reducesExplosion(true, true));
	}

	@Test
	@DisplayName("an explosion without a push has nothing to reduce")
	void explosionWithoutAPushIsLeftAlone() {
		assertFalse(KnockbackFilter.reducesExplosion(true, false));
	}

	@Test
	@DisplayName("a disabled module leaves every explosion alone, with or without a push")
	void disabledModuleLeavesTheExplosionAlone() {
		assertFalse(KnockbackFilter.reducesExplosion(false, true));
		assertFalse(KnockbackFilter.reducesExplosion(false, false));
	}
}

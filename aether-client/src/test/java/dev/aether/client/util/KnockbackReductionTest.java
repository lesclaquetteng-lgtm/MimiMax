package dev.aether.client.util;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * The arithmetic of the anti knockback module, checked without the game.
 *
 * <p>These assertions are the proof that the module changes the knockback: the module hands its
 * settings to this class at the tail of the vanilla knockback, so a broken calculation here is a
 * module that does nothing or, worse, a hit harder than vanilla.</p>
 */
class KnockbackReductionTest {

	/** The delta vanilla writes down for a sword hit: 0.4 backwards, 0.4 up. */
	private static final double[] VANILLA = {0.4, 0.4, -0.2};

	@Test
	@DisplayName("the defaults cancel the horizontal knockback and keep the vertical one")
	void defaultsCancelHorizontal() {
		double[] reduced = KnockbackReduction.of(true, 0.0, 1.0).apply(VANILLA[0], VANILLA[1], VANILLA[2]);

		assertArrayEquals(new double[] {0.0, 0.4, 0.0}, reduced, 1e-9);
	}

	@Test
	@DisplayName("the factors scale both ground axes and the vertical one")
	void factorsScaleEachAxis() {
		double[] reduced = KnockbackReduction.of(true, 0.5, 0.25).apply(VANILLA[0], VANILLA[1], VANILLA[2]);

		assertArrayEquals(new double[] {0.2, 0.1, -0.1}, reduced, 1e-9);
	}

	@Test
	@DisplayName("a disabled module gives the delta back untouched")
	void disabledLeavesTheDeltaAlone() {
		double[] reduced = KnockbackReduction.of(false, 0.0, 0.0).apply(VANILLA[0], VANILLA[1], VANILLA[2]);

		assertArrayEquals(VANILLA, reduced, 0.0);
	}

	@Test
	@DisplayName("a saved factor can never make the knockback stronger than vanilla")
	void factorsAreClamped() {
		double[] above = KnockbackReduction.of(true, 3.0, 2.0).apply(VANILLA[0], VANILLA[1], VANILLA[2]);
		double[] below = KnockbackReduction.of(true, -1.0, -0.5).apply(VANILLA[0], VANILLA[1], VANILLA[2]);

		assertArrayEquals(VANILLA, above, 1e-9);
		assertArrayEquals(new double[] {0.0, 0.0, 0.0}, below, 1e-9);
	}

	@Test
	@DisplayName("a hand-edited factor that is not a number keeps the vanilla knockback")
	void notANumberFallsBackToVanilla() {
		KnockbackReduction reduction = KnockbackReduction.of(true, Double.NaN, Double.NaN);

		assertEquals(1.0, reduction.horizontal(), 0.0);
		assertEquals(1.0, reduction.vertical(), 0.0);
		assertArrayEquals(VANILLA, reduction.apply(VANILLA[0], VANILLA[1], VANILLA[2]), 0.0);
	}

	@Test
	@DisplayName("a knockback of zero stays zero, whatever the factors")
	void zeroStaysZero() {
		double[] reduced = KnockbackReduction.of(true, 0.5, 0.5).apply(0.0, 0.0, 0.0);

		assertArrayEquals(new double[] {0.0, 0.0, 0.0}, reduced, 0.0);
	}

	/** The push an explosion adds, and the motion the player already had when it landed. */
	private static final double[] PUSH = {0.6, 0.5, -0.2};
	private static final double[] MOTION = {0.1, -0.08, 0.05};

	@Test
	@DisplayName("an added push is replaced by its reduced part, the rest of the motion is untouched")
	void addedPushIsReplaced() {
		double[] reduced = KnockbackReduction.of(true, 0.0, 1.0)
				.applyToAdded(PUSH[0], PUSH[1], PUSH[2], MOTION[0], MOTION[1], MOTION[2]);

		// The walk and the fall keep their direction, the ground part of the push is gone.
		assertArrayEquals(new double[] {-0.5, -0.08, 0.25}, reduced, 1e-9);
	}

	@Test
	@DisplayName("the factors scale an added push exactly as they scale a replaced delta")
	void addedPushObeysTheSameFactors() {
		KnockbackReduction reduction = KnockbackReduction.of(true, 0.5, 0.25);

		double[] added = reduction.applyToAdded(PUSH[0], PUSH[1], PUSH[2], MOTION[0], MOTION[1], MOTION[2]);
		double[] scaled = reduction.apply(PUSH[0], PUSH[1], PUSH[2]);

		assertArrayEquals(new double[] {-0.2, -0.455, 0.15}, added, 1e-9);
		assertArrayEquals(new double[] {
				MOTION[0] - PUSH[0] + scaled[0],
				MOTION[1] - PUSH[1] + scaled[1],
				MOTION[2] - PUSH[2] + scaled[2]}, added, 1e-12);
	}

	@Test
	@DisplayName("a disabled module leaves the motion that already holds the push alone")
	void addedPushOfADisabledModuleIsLeftAlone() {
		double[] reduced = KnockbackReduction.of(false, 0.0, 0.0)
				.applyToAdded(PUSH[0], PUSH[1], PUSH[2], MOTION[0], MOTION[1], MOTION[2]);

		assertArrayEquals(MOTION, reduced, 0.0);
	}

	@Test
	@DisplayName("a push that is not finite is ignored, as vanilla ignores it")
	void notFinitePushIsIgnored() {
		// Entity#addDeltaMovement refuses the whole vector, so it never reached the motion: taking
		// it back out would turn the delta into NaN.
		double[] reduced = KnockbackReduction.of(true, 0.0, 0.0)
				.applyToAdded(Double.NaN, PUSH[1], PUSH[2], MOTION[0], MOTION[1], MOTION[2]);

		assertArrayEquals(MOTION, reduced, 0.0);
	}

	@Test
	@DisplayName("a saved factor above one leaves an added push as vanilla")
	void addedPushIsClamped() {
		double[] reduced = KnockbackReduction.of(true, 3.0, 2.0)
				.applyToAdded(PUSH[0], PUSH[1], PUSH[2], MOTION[0], MOTION[1], MOTION[2]);

		assertArrayEquals(MOTION, reduced, 1e-12);
	}
}

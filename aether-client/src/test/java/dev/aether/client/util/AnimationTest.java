package dev.aether.client.util;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Tests the animation used by the HUD and the click GUI.
 *
 * <p>The motion has to look the same at 30 and at 240 frames per second, and a caller who forgets
 * the speed must still get a moving animation: both were silent traps before.</p>
 */
class AnimationTest {

	@Test
	@DisplayName("an animation built without a speed still moves")
	void defaultSpeedMoves() {
		Animation animation = new Animation(0.0f);
		assertEquals(Animation.DEFAULT_SPEED, animation.getSpeed(), 0.0001f);
		animation.setTarget(1.0f);
		animation.update(16L);
		assertTrue(animation.getValue() > 0.0f, "the animation did not move");
	}

	@Test
	@DisplayName("two half frames land where one whole frame does")
	void frameRateIndependent() {
		Animation fortyAtSixteen = new Animation(0.0f, 12.0f);
		fortyAtSixteen.setTarget(1.0f);
		fortyAtSixteen.update(16L);

		Animation twoAtEight = new Animation(0.0f, 12.0f);
		twoAtEight.setTarget(1.0f);
		twoAtEight.update(8L);
		twoAtEight.update(8L);

		assertEquals(fortyAtSixteen.getValue(), twoAtEight.getValue(), 0.0001f,
				"the value depends on the frame rate");
	}

	@Test
	@DisplayName("a negative delta is ignored")
	void negativeDeltaIsIgnored() {
		Animation animation = new Animation(0.5f, 12.0f);
		animation.setTarget(1.0f);
		float before = animation.getValue();
		animation.update(-16L);
		assertEquals(before, animation.getValue(), 0.0f);
		animation.update(0L);
		assertEquals(before, animation.getValue(), 0.0f);
	}

	@Test
	@DisplayName("an animation reaches both ends without overshooting")
	void reachesBothEnds() {
		Animation up = new Animation(0.0f, 14.0f);
		up.setTarget(1.0f);
		for (int frame = 0; frame < 120; frame++) {
			up.update(16L);
			assertTrue(up.getValue() <= 1.0f, "the value overshot its target");
		}
		assertTrue(up.isFinished());

		Animation down = new Animation(1.0f, 14.0f);
		down.setTarget(0.0f);
		for (int frame = 0; frame < 120; frame++) {
			down.update(16L);
			assertTrue(down.getValue() >= 0.0f, "the value fell under its target");
		}
		assertTrue(down.isFinished());
	}

	@Test
	@DisplayName("an animation is not finished while it still travels")
	void stillRunning() {
		Animation animation = new Animation(0.0f, 4.0f);
		animation.setTarget(1.0f);
		animation.update(16L);
		assertFalse(animation.isFinished());
	}

	@Test
	@DisplayName("a speed of zero falls back on the default one instead of freezing")
	void zeroSpeedFallsBack() {
		Animation animation = new Animation(0.0f, 0.0f);
		assertEquals(Animation.DEFAULT_SPEED, animation.getSpeed(), 0.0001f);
		animation.setTarget(1.0f);
		animation.update(16L);
		assertTrue(animation.getValue() > 0.0f);
	}
}

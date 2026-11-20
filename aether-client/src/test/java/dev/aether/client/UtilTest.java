package dev.aether.client;

import dev.aether.client.util.Animation;
import dev.aether.client.util.ColorUtil;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Tests the small helpers of the client.
 */
class UtilTest {

	@Test
	@DisplayName("colours are packed and unpacked correctly")
	void colourPacking() {
		assertEquals(0xFF6C5CE7, ColorUtil.rgb(0x6C, 0x5C, 0xE7));
		assertEquals(0xFF6C5CE7, ColorUtil.fromHex("#6C5CE7", 0));
		assertEquals(0xFF6C5CE7, ColorUtil.fromHex("6c5ce7", 0));
		assertEquals(0x806C5CE7, ColorUtil.fromHex("#806C5CE7", 0));
		assertEquals("#6C5CE7", ColorUtil.toHex(0xFF6C5CE7));
		assertEquals(0x11223344, ColorUtil.fromHex("broken", 0x11223344));
	}

	@Test
	@DisplayName("the alpha channel can be changed")
	void alphaChannel() {
		int opaque = 0xFF6C5CE7;
		assertEquals(0x806C5CE7, ColorUtil.withAlpha(opaque, 0.5f));
		assertEquals(0x006C5CE7, ColorUtil.withAlpha(opaque, 0.0f));
		assertEquals(0xFF6C5CE7, ColorUtil.withAlpha(opaque, 1.0f));
	}

	@Test
	@DisplayName("a colour fades without losing its own alpha")
	void colourFade() {
		// An opaque surface at half opacity.
		assertEquals(0x806C5CE7, ColorUtil.fade(0xFF6C5CE7, 0.5f));
		// A surface that is already translucent keeps its weight at full opacity...
		assertEquals(0xF2141417, ColorUtil.fade(0xF2141417, 1.0f));
		// ...and fades from its own alpha, not from 255.
		assertEquals(0x79141417, ColorUtil.fade(0xF2141417, 0.5f));
		assertEquals(0x00141417, ColorUtil.fade(0xF2141417, 0.0f));
		// Out of range factors are clamped.
		assertEquals(0xF2141417, ColorUtil.fade(0xF2141417, 3.0f));
		assertEquals(0x00141417, ColorUtil.fade(0xF2141417, -1.0f));
	}

	@Test
	@DisplayName("colours are interpolated")
	void colourInterpolation() {
		assertEquals(0xFF000000, ColorUtil.lerp(0xFF000000, 0xFFFFFFFF, 0.0f));
		assertEquals(0xFFFFFFFF, ColorUtil.lerp(0xFF000000, 0xFFFFFFFF, 1.0f));
		assertEquals(0xFF808080, ColorUtil.lerp(0xFF000000, 0xFFFFFFFF, 0.5f));
	}

	@Test
	@DisplayName("an animation converges towards its target")
	void animationConverges() {
		Animation animation = new Animation(0.0f, 10.0f);
		animation.setTarget(1.0f);
		for (int i = 0; i < 200; i++) {
			animation.update(16L);
		}
		assertTrue(animation.isFinished(), "the animation did not finish");
		assertEquals(1.0f, animation.getValue(), 0.001f);
	}

	@Test
	@DisplayName("an animation can be stopped")
	void animationCanBeStopped() {
		Animation animation = new Animation(0.5f, 10.0f);
		animation.set(0.75f);
		assertEquals(0.75f, animation.getValue(), 0.0001f);
		assertEquals(0.75f, animation.getTarget(), 0.0001f);
	}
}

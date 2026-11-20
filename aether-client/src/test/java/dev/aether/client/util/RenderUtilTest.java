package dev.aether.client.util;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

/**
 * Tests the chord cache behind the rounded rectangles.
 *
 * <p>The cache must return exactly the chords the per-column square root produced before it
 * existed: the shape of every panel of the client depends on it.</p>
 */
class RenderUtilTest {

	/** The formula the renderer used before the cache, kept here as the reference. */
	private static int[] reference(int radius) {
		int[] chords = new int[radius];
		for (int i = 0; i < radius; i++) {
			double distance = radius - i - 0.5;
			chords[i] = (int) Math.round(
					Math.sqrt(Math.max(0.0, radius * radius - distance * distance)));
		}
		return chords;
	}

	@Test
	@DisplayName("the cached chords match the square root formula for every radius of the interface")
	void chordsMatchTheFormula() {
		for (int radius = 1; radius <= 64; radius++) {
			assertArrayEquals(reference(radius), RenderUtil.chords(radius),
					"the chords of radius " + radius + " changed");
		}
	}

	@Test
	@DisplayName("a radius is only computed once")
	void chordsAreCached() {
		int[] first = RenderUtil.chords(7);
		int[] second = RenderUtil.chords(7);
		assertSame(first, second, "the cache returned a new array");
	}

	@Test
	@DisplayName("the innermost column reaches the full radius and a zero radius is empty")
	void chordBounds() {
		int[] chords = RenderUtil.chords(5);
		assertEquals(5, chords[chords.length - 1], "the centre column must close the corner");
		assertEquals(0, RenderUtil.chords(0).length);
	}

	@Test
	@DisplayName("a radius beyond the cache is still correct")
	void largeRadiusFallsBack() {
		assertArrayEquals(reference(100), RenderUtil.chords(100));
	}
}
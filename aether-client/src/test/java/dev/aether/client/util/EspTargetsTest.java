package dev.aether.client.util;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The ESP selection and, above all, the entities it has to release.
 *
 * <p>The glow of an entity is client state: setting it leaves it set. These assertions are the
 * proof that an entity leaving the range, or the module being turned off, gives the glow back
 * instead of leaving a ghost highlighted for the rest of the session.</p>
 */
class EspTargetsTest {

	private static Set<Integer> ids(Integer... values) {
		return new LinkedHashSet<>(Arrays.asList(values));
	}

	@Test
	@DisplayName("the Players mode selects players and nothing else")
	void playersModeSelectsPlayers() {
		assertTrue(EspTargets.selects("Players", true, 10.0));
		assertFalse(EspTargets.selects("Players", false, 10.0));
	}

	@Test
	@DisplayName("the Mobs mode selects everything but the players")
	void mobsModeSelectsMobs() {
		assertFalse(EspTargets.selects("Mobs", true, 10.0));
		assertTrue(EspTargets.selects("Mobs", false, 10.0));
	}

	@Test
	@DisplayName("the All mode selects both, inside the range only")
	void allModeSelectsInsideTheRange() {
		assertTrue(EspTargets.selects("All", true, EspTargets.RANGE - 0.5));
		assertTrue(EspTargets.selects("All", false, 0.0));
		assertFalse(EspTargets.selects("All", true, EspTargets.RANGE));
		assertFalse(EspTargets.selects("All", true, Double.NaN), "an unknown position is not highlighted");
	}

	@Test
	@DisplayName("squared distances preserve the ESP range boundary without a square root")
	void squaredDistanceSelectionUsesTheSameStrictBoundary() {
		assertTrue(EspTargets.selectsSquared("All", true, (EspTargets.RANGE - 0.5) * (EspTargets.RANGE - 0.5)));
		assertFalse(EspTargets.selectsSquared("All", true, EspTargets.RANGE_SQUARED));
		assertFalse(EspTargets.selectsSquared("All", true, Double.NaN));
	}

	@Test
	@DisplayName("ESP leaves glow tags owned by other systems untouched")
	void onlyOwnsGlowItAddedItself() {
		assertTrue(EspTargets.shouldTrackGlow(false, false), "a new glow is owned by ESP");
		assertFalse(EspTargets.shouldTrackGlow(false, true), "an existing game glow is not adopted");
		assertTrue(EspTargets.shouldTrackGlow(true, true), "ESP keeps tracking its own active glow");
		assertTrue(EspTargets.shouldTrackGlow(true, false), "a removed ESP glow is restored on the next tick");
	}

	@Test
	@DisplayName("an entity that leaves the range is released")
	void leavingTheRangeIsReleased() {
		assertEquals(ids(2), EspTargets.stale(ids(1, 2, 3), ids(1, 3)));
	}

	@Test
	@DisplayName("turning the module off releases every entity it tagged")
	void disablingReleasesEverything() {
		assertEquals(ids(4, 5), EspTargets.stale(ids(4, 5), Set.of()));
	}

	@Test
	@DisplayName("an entity that still matches keeps its glow")
	void nothingIsReleasedWhenNothingLeft() {
		assertTrue(EspTargets.stale(ids(1, 2), ids(1, 2, 3)).isEmpty());
		assertTrue(EspTargets.stale(Set.of(), ids(1)).isEmpty());
	}
}

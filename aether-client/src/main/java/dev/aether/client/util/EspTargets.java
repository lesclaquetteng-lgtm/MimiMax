package dev.aether.client.util;

import java.util.LinkedHashSet;
import java.util.Set;

/**
 * The selection of the ESP module, without a single Minecraft type.
 *
 * <p>The glow of an entity is client state: setting it leaves it set. The module used to tag every
 * entity in range and never take the tag back, so an entity that walked out of range, or that the
 * module was turned off on, stayed highlighted for the rest of the session — and turning the module
 * off cleared the glow of every entity in the level, including the ones the server had made glow
 * with a spectral arrow.</p>
 *
 * <p>{@link #stale} names exactly the entities a selection has to release, which is what makes the
 * cleanup testable out of game.</p>
 */
public final class EspTargets {

	/** The distance, in blocks, inside which an entity is highlighted. */
	public static final double RANGE = 64.0;
	/** Squared form avoids a square root for every entity scanned by the tick callback. */
	public static final double RANGE_SQUARED = RANGE * RANGE;

	private EspTargets() {
	}

	/**
	 * Decides whether an entity is selected by the module.
	 *
	 * @param mode     the option of the module: {@code Players}, {@code Mobs} or {@code All}
	 * @param player   whether the entity is a player
	 * @param distance the distance to the local player, in blocks
	 * @return whether the entity has to glow
	 */
	public static boolean selects(String mode, boolean player, double distance) {
		return selectsSquared(mode, player, distance * distance);
	}

	/** Same selection using squared distance, for hot loops that already have a squared metric. */
	public static boolean selectsSquared(String mode, boolean player, double distanceSquared) {
		if (!(distanceSquared < RANGE_SQUARED)) {
			// Also rejects a distance that is not a number: an entity at an unknown position has no
			// business being highlighted.
			return false;
		}
		if ("Players".equals(mode)) {
			return player;
		}
		if ("Mobs".equals(mode)) {
			return !player;
		}
		return true;
	}

	/** Whether ESP should own a glow tag, preserving glow state that another system already set. */
	public static boolean shouldTrackGlow(boolean alreadyTracked, boolean alreadyGlowing) {
		return alreadyTracked || !alreadyGlowing;
	}

	/**
	 * @param previous the entities that are glowing because of the module
	 * @param current  the entities the module selects now
	 * @return the entities to unmark, in the order they were tagged
	 */
	public static Set<Integer> stale(Set<Integer> previous, Set<Integer> current) {
		Set<Integer> stale = new LinkedHashSet<>();
		for (int id : previous) {
			if (!current.contains(id)) {
				stale.add(id);
			}
		}
		return stale;
	}
}

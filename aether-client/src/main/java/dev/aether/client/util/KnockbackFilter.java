package dev.aether.client.util;

/**
 * The conditions under which the anti knockback module may touch a packet.
 *
 * <p>The mixin used to spell them out inline: a packet naming another entity, a module that is off
 * and an explosion without a push all ended in the same {@code return}. Those decisions live here,
 * without a single Minecraft type, so they can be tested without a game — a wrong comparison would
 * either leave the player's knockback untouched or wipe the velocity of an unrelated entity.</p>
 *
 * <p>Only the decision is pure: the hooks still read the packet, the player and the module, and do
 * the writing themselves.</p>
 */
public final class KnockbackFilter {

	private KnockbackFilter() {
	}

	/**
	 * Decides whether a velocity packet belongs to the local player and may be corrected.
	 *
	 * <p>The comparison is exact: a packet for another entity keeps its vanilla meaning, and the id
	 * of the local player is the only one the module is allowed to rewrite.</p>
	 *
	 * @param moduleEnabled  whether the module is registered and enabled
	 * @param packetEntityId the entity the packet names
	 * @param playerEntityId the entity id of the local player
	 * @return whether the delta of that packet may be reduced
	 */
	public static boolean reducesMotion(boolean moduleEnabled, int packetEntityId, int playerEntityId) {
		return moduleEnabled && packetEntityId == playerEntityId;
	}

	/**
	 * Decides whether an explosion packet carries a push the module may correct.
	 *
	 * <p>Vanilla applies the push to the local player and to nobody else, so there is no id to
	 * compare here: the packet either carries a push or carries none, and a packet without one has
	 * nothing to reduce.</p>
	 *
	 * @param moduleEnabled whether the module is registered and enabled
	 * @param pushPresent   whether the packet carries a knockback vector
	 * @return whether the push of that packet may be reduced
	 */
	public static boolean reducesExplosion(boolean moduleEnabled, boolean pushPresent) {
		return moduleEnabled && pushPresent;
	}
}

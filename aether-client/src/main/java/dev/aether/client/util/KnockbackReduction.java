package dev.aether.client.util;

/**
 * The part of the knockback the player keeps, as a factor of the vanilla one.
 *
 * <p>The anti knockback module used to expose its factors without anything applying them: the
 * settings were displayed, saved, and read by nobody, so the module changed nothing at all. The
 * arithmetic lives here, without a single Minecraft type, so that it can be tested off game.</p>
 *
 	 * <p>The horizontal factor scales both {@code x} and {@code z}: a knockback is a direction on the
	 * ground, and splitting one axis from the other would let a hit slide sideways.</p>
	 *
	 * <p>Two shapes of knockback reach the client, and both go through this class: a hit replaces the
	 * velocity ({@link #apply}), an explosion adds a push to it ({@link #applyToAdded}).</p>
 */
public final class KnockbackReduction {

	/** A factor of one leaves the vanilla delta untouched, zero cancels that axis. */
	private static final double UNCHANGED = 1.0;
	private static final double CANCELLED = 0.0;

	private final boolean active;
	private final double horizontal;
	private final double vertical;

	private KnockbackReduction(boolean active, double horizontal, double vertical) {
		this.active = active;
		this.horizontal = horizontal;
		this.vertical = vertical;
	}

	/**
	 * Builds the reduction of one knockback.
	 *
	 * @param active     whether the module is enabled; disabled means the input comes back untouched
	 * @param horizontal the part of the horizontal knockback that is kept, clamped to {@code [0, 1]}
	 * @param vertical   the part of the vertical knockback that is kept, clamped to {@code [0, 1]}
	 * @return the reduction to apply
	 */
	public static KnockbackReduction of(boolean active, double horizontal, double vertical) {
		return new KnockbackReduction(active, clamp(horizontal), clamp(vertical));
	}

	/**
	 * @return the delta to apply, in the order {@code x}, {@code y}, {@code z}
	 */
	public double[] apply(double deltaX, double deltaY, double deltaZ) {
		if (!active) {
			return new double[] {deltaX, deltaY, deltaZ};
		}
		return new double[] {deltaX * horizontal, deltaY * vertical, deltaZ * horizontal};
	}

	/**
	 * Corrects a push that vanilla has already added to the current motion.
	 *
	 * <p>An explosion does not replace the velocity of the player, it adds its push to it:
	 * {@code ClientPacketListener#handleExplosion} calls {@code addDeltaMovement}, so by the time
	 * the mixin runs the delta carries the push plus whatever walking or falling was already
	 * there. Taking the push back out and adding the reduced one leaves the rest of the motion
	 * exactly as vanilla wrote it.</p>
	 *
	 * <p>Vanilla refuses a push that is not finite ({@code Entity#addDeltaMovement} checks
	 * {@code Vec3#isFinite} before adding anything), so such a push is not in the current motion
	 * and subtracting it would corrupt the delta. It is ignored, like vanilla ignores it.</p>
	 *
	 * @param addedX   the horizontal push vanilla added
	 * @param addedY   the vertical push vanilla added
	 * @param addedZ   the second horizontal push vanilla added
	 * @param currentX the horizontal motion the player has, push included
	 * @param currentY the vertical motion the player has, push included
	 * @param currentZ the second horizontal motion the player has, push included
	 * @return the delta to apply, in the order {@code x}, {@code y}, {@code z}
	 */
	public double[] applyToAdded(double addedX, double addedY, double addedZ,
			double currentX, double currentY, double currentZ) {
		if (!active || !isFinite(addedX, addedY, addedZ)) {
			return new double[] {currentX, currentY, currentZ};
		}
		return new double[] {
				currentX - addedX * (UNCHANGED - horizontal),
				currentY - addedY * (UNCHANGED - vertical),
				currentZ - addedZ * (UNCHANGED - horizontal)
		};
	}

	public double horizontal() {
		return horizontal;
	}

	public double vertical() {
		return vertical;
	}

	/** Mirrors the guard of {@code Entity#addDeltaMovement}, which only adds a finite vector. */
	private static boolean isFinite(double x, double y, double z) {
		return Double.isFinite(x) && Double.isFinite(y) && Double.isFinite(z);
	}

	/**
	 * Keeps a saved value from ever amplifying a knockback: the module promises a reduction, and a
	 * negative or above-one factor would be a hit harder than vanilla. A value that is not a number
	 * (a hand-edited configuration file) keeps the vanilla knockback.
	 */
	private static double clamp(double factor) {
		if (Double.isNaN(factor)) {
			return UNCHANGED;
		}
		return Math.max(CANCELLED, Math.min(UNCHANGED, factor));
	}
}

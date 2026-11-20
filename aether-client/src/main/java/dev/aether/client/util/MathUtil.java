package dev.aether.client.util;

/**
 * Small collection of maths helpers used by the animations and the widgets.
 */
public final class MathUtil {
	private MathUtil() {
	}

	/**
	 * Clamps a value between a minimum and a maximum.
	 */
	public static double clamp(double value, double min, double max) {
		return Math.max(min, Math.min(max, value));
	}

	/**
	 * Clamps a float between a minimum and a maximum.
	 */
	public static float clamp(float value, float min, float max) {
		return Math.max(min, Math.min(max, value));
	}

	/**
	 * Clamps an int between a minimum and a maximum.
	 */
	public static int clamp(int value, int min, int max) {
		return Math.max(min, Math.min(max, value));
	}

	/**
	 * Linearly interpolates between two values.
	 */
	public static double lerp(double from, double to, double factor) {
		return from + (to - from) * factor;
	}

	/**
	 * Maps a value from one range into another.
	 */
	public static double map(double value, double inMin, double inMax, double outMin, double outMax) {
		if (inMax - inMin == 0.0) {
			return outMin;
		}
		return outMin + (value - inMin) * (outMax - outMin) / (inMax - inMin);
	}

	/**
	 * Rounds a value to the given number of decimals.
	 */
	public static double round(double value, int decimals) {
		double factor = Math.pow(10, decimals);
		return Math.round(value * factor) / factor;
	}
}

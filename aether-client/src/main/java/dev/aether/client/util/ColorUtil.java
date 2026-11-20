package dev.aether.client.util;

import java.util.Locale;

/**
 * Small helper around the packed ARGB integers Minecraft uses for colours.
 */
public final class ColorUtil {
	private ColorUtil() {
	}

	/**
	 * Packs a red, green and blue value into an opaque colour.
	 * @param r 0-255
	 * @param g 0-255
	 * @param b 0-255
	 */
	public static int rgb(int r, int g, int b) {
		return 0xFF000000 | (clamp(r) << 16) | (clamp(g) << 8) | clamp(b);
	}

	/**
	 * Applies an alpha value (0.0 - 1.0) to an existing colour.
	 */
	public static int withAlpha(int color, float alpha) {
		int a = Math.round(Math.max(0.0f, Math.min(1.0f, alpha)) * 255.0f);
		return (a << 24) | (color & 0x00FFFFFF);
	}

	/**
	 * Fades a colour: its own alpha is multiplied by the factor, so a panel that is already
	 * translucent stays as light as it was designed to be.
	 *
	 * @param factor 0.0 (invisible) to 1.0 (unchanged)
	 */
	public static int fade(int color, float factor) {
		float clamped = Math.max(0.0f, Math.min(1.0f, factor));
		return withAlpha(color, (alpha(color) / 255.0f) * clamped);
	}

	/**
	 * Interpolates between two colours.
	 */
	public static int lerp(int from, int to, float factor) {
		float t = Math.max(0.0f, Math.min(1.0f, factor));
		int a = Math.round(((from >>> 24) & 0xFF) + (((to >>> 24) & 0xFF) - ((from >>> 24) & 0xFF)) * t);
		int r = Math.round(((from >> 16) & 0xFF) + (((to >> 16) & 0xFF) - ((from >> 16) & 0xFF)) * t);
		int g = Math.round(((from >> 8) & 0xFF) + (((to >> 8) & 0xFF) - ((from >> 8) & 0xFF)) * t);
		int b = Math.round((from & 0xFF) + ((to & 0xFF) - (from & 0xFF)) * t);
		return (a << 24) | (r << 16) | (g << 8) | b;
	}

	/**
	 * Blends two colours with the given factor.
	 */
	public static int blend(int from, int to, float factor) {
		return lerp(from, to, factor);
	}

	/**
	 * Extracts the alpha channel.
	 */
	public static int alpha(int color) {
		return (color >>> 24) & 0xFF;
	}

	/**
	 * Converts a hex string such as {@code #6C5CE7} into a packed colour.
	 * Falls back to the given default when the string cannot be parsed.
	 */
	public static int fromHex(String hex, int fallback) {
		if (hex == null) {
			return fallback;
		}
		String cleaned = hex.trim().toLowerCase(Locale.ROOT);
		if (cleaned.startsWith("#")) {
			cleaned = cleaned.substring(1);
		}
		if (cleaned.length() == 6) {
			cleaned = "ff" + cleaned;
		}
		if (cleaned.length() != 8) {
			return fallback;
		}
		try {
			return (int) Long.parseLong(cleaned, 16);
		} catch (NumberFormatException exception) {
			return fallback;
		}
	}

	/**
	 * Converts a packed colour into a hex string such as {@code #6C5CE7}.
	 */
	public static String toHex(int color) {
		return String.format("#%06X", color & 0xFFFFFF);
	}

	private static int clamp(int value) {
		return Math.max(0, Math.min(255, value));
	}
}

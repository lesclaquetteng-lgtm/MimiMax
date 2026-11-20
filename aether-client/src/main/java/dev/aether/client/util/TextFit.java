package dev.aether.client.util;

import java.util.function.ToIntFunction;

/**
 * Fits a string into a width without drawing it.
 *
 * <p>Kept apart from {@link RenderUtil} because the algorithm knows nothing about the game: it asks
 * a measuring function for the width of a string, so it can be exercised — and its cost counted — in
 * a unit test without a client.</p>
 */
public final class TextFit {
	/** The character that marks a shortened string. */
	public static final String ELLIPSIS = "…";

	private TextFit() {
	}

	/**
	 * Shortens a string until it fits, appending an ellipsis.
	 *
	 * <p>The longest prefix that still fits is found by dichotomy: about {@code log2(length)} width
	 * measurements instead of one per removed character. A string that already fits is returned
	 * untouched, and at least one character is kept — like the character-by-character version this
	 * replaces.</p>
	 *
	 * @param maxWidth the width available, in the unit of the measuring function
	 * @param measure the width of a string, in the same unit
	 */
	public static String truncate(String value, int maxWidth, ToIntFunction<String> measure) {
		if (measure.applyAsInt(value) <= maxWidth) {
			return value;
		}

		int low = 1;
		int high = value.length();
		int fitting = 0;

		while (low <= high) {
			int middle = (low + high) >>> 1;
			if (measure.applyAsInt(value.substring(0, middle) + ELLIPSIS) <= maxWidth) {
				fitting = middle;
				low = middle + 1;
			} else {
				high = middle - 1;
			}
		}

		int length = Math.min(value.length(), Math.max(1, fitting));
		return value.substring(0, length) + ELLIPSIS;
	}
}

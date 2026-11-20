package dev.aether.client.util;

import java.util.function.IntUnaryOperator;

/**
 * Groups the samples of a small bar chart into rectangles.
 *
 * <p>Adjacent bars of the same height and the same colour become one rectangle, so a steady value
 * costs a handful of fills instead of one per sample. The grouping is pure — the colour comes from
 * a function — so it is tested and measured without the game.</p>
 */
public final class BarChart {
	private BarChart() {
	}

	/**
	 * Groups the samples into runs of adjacent bars of the same height and colour.
	 *
	 * @param values one value per bar, oldest first
	 * @param peak   the value that fills the chart
	 * @param height the height of the chart in pixels
	 * @param colour the colour of a bar, given its value; a bar of height zero carries no colour
	 * @param out    at least {@code values.length * 4} ints, filled with from, to, height, colour
	 * @return the number of runs written into {@code out}
	 */
	public static int runs(int[] values, int peak, int height, IntUnaryOperator colour, int[] out) {
		int count = 0;
		int from = 0;
		int runHeight = -1;
		int runColour = 0;
		int divisor = Math.max(1, peak);

		for (int index = 0; index < values.length; index++) {
			int value = values[index];
			int bar = Math.max(0, Math.min(height, value * height / divisor));
			int barColour = bar == 0 ? 0 : colour.applyAsInt(value);
			if (index > 0 && (bar != runHeight || barColour != runColour)) {
				count = write(out, count, from, index, runHeight, runColour);
				from = index;
			}
			runHeight = bar;
			runColour = barColour;
		}

		if (values.length > 0) {
			count = write(out, count, from, values.length, runHeight, runColour);
		}
		return count;
	}

	private static int write(int[] out, int count, int from, int to, int height, int colour) {
		if (to <= from) {
			return count;
		}
		out[count * 4] = from;
		out[count * 4 + 1] = to;
		out[count * 4 + 2] = height;
		out[count * 4 + 3] = colour;
		return count + 1;
	}
}

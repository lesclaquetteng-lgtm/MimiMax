package dev.aether.client;

import dev.aether.client.util.BarChart;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Arrays;
import java.util.Random;
import java.util.function.IntUnaryOperator;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Tests the bar grouping of the FPS graph: every sample must be covered exactly once, with the
 * same height as before, in as few rectangles as the samples allow.
 */
class BarChartTest {
	private static final int SAMPLES = 54;
	private static final int HEIGHT = 15;
	/** The colours of the client, in the order the frame rate reaches them. */
	private static final IntUnaryOperator COLOUR =
			value -> value >= 120 ? 0xFF2ECC8F : value >= 60 ? 0xFFF0B429 : 0xFFE05260;

	private static int[] buffer() {
		return new int[SAMPLES * 4];
	}

	private static int[] steady(int value) {
		int[] values = new int[SAMPLES];
		Arrays.fill(values, value);
		return values;
	}

	@Test
	@DisplayName("a steady value is a single rectangle")
	void aSteadyValueIsOneRectangle() {
		int[] out = buffer();
		int count = BarChart.runs(steady(90), 90, HEIGHT, COLOUR, out);

		assertEquals(1, count);
		assertEquals(0, out[0]);
		assertEquals(SAMPLES, out[1]);
		assertEquals(HEIGHT, out[2]);
		assertEquals(COLOUR.applyAsInt(90), out[3]);
	}

	@Test
	@DisplayName("a value that changes every sample keeps one rectangle per sample")
	void everyStepIsItsOwnRectangle() {
		int[] values = new int[SAMPLES];
		for (int index = 0; index < SAMPLES; index++) {
			values[index] = index % 2 == 0 ? 30 : 90;
		}

		int[] out = buffer();
		int count = BarChart.runs(values, 90, HEIGHT, COLOUR, out);

		assertEquals(SAMPLES, count);
		for (int run = 0; run < count; run++) {
			assertEquals(run, out[run * 4]);
			assertEquals(run + 1, out[run * 4 + 1]);
		}
	}

	@Test
	@DisplayName("a plateau followed by a drop makes two rectangles")
	void aPlateauThenADrop() {
		int[] values = new int[SAMPLES];
		for (int index = 0; index < 10; index++) {
			values[index] = 60;
		}
		for (int index = 10; index < SAMPLES; index++) {
			values[index] = 30;
		}

		int[] out = buffer();
		int count = BarChart.runs(values, 60, HEIGHT, COLOUR, out);

		assertEquals(2, count);
		assertEquals(0, out[0]);
		assertEquals(10, out[1]);
		assertEquals(HEIGHT, out[2]);
		assertEquals(10, out[4]);
		assertEquals(SAMPLES, out[5]);
		assertEquals(HEIGHT / 2, out[6]);
	}

	@Test
	@DisplayName("samples that do not reach the chart are one empty rectangle")
	void emptySamplesAreOneRun() {
		int[] out = buffer();
		int count = BarChart.runs(new int[SAMPLES], 60, HEIGHT, COLOUR, out);

		assertEquals(1, count);
		assertEquals(0, out[2], "the height of the run is zero, so nothing is drawn");
		assertEquals(0, out[3], "and it carries no colour");
	}

	@Test
	@DisplayName("the runs cover every sample exactly once, oldest first")
	void theRunsCoverEverySampleOnce() {
		Random random = new Random(20261115L);
		int[] values = new int[SAMPLES];
		for (int index = 0; index < SAMPLES; index++) {
			values[index] = random.nextInt(200);
		}

		int[] out = buffer();
		int count = BarChart.runs(values, 200, HEIGHT, COLOUR, out);

		int position = 0;
		for (int run = 0; run < count; run++) {
			int from = out[run * 4];
			int to = out[run * 4 + 1];
			assertEquals(position, from, "the runs are contiguous");
			assertTrue(to > from, "a run is never empty");
			position = to;
		}
		assertEquals(SAMPLES, position, "the last run ends on the last sample");
	}

	@Test
	@DisplayName("a run keeps the height of every sample it covers")
	void aRunKeepsTheHeightOfItsSamples() {
		Random random = new Random(7L);
		int[] values = new int[SAMPLES];
		for (int index = 0; index < SAMPLES; index++) {
			values[index] = random.nextInt(160);
		}

		int[] out = buffer();
		int count = BarChart.runs(values, 160, HEIGHT, COLOUR, out);

		for (int run = 0; run < count; run++) {
			int height = out[run * 4 + 2];
			for (int index = out[run * 4]; index < out[run * 4 + 1]; index++) {
				int expected = Math.min(HEIGHT, values[index] * HEIGHT / 160);
				assertEquals(expected, height, "sample " + index);
			}
		}
	}

	@Test
	@DisplayName("the merged chart never uses more rectangles than there are samples")
	void theCountNeverExceedsTheSamples() {
		Random random = new Random(99L);
		int[] out = buffer();
		for (int round = 0; round < 200; round++) {
			int[] values = new int[SAMPLES];
			for (int index = 0; index < SAMPLES; index++) {
				values[index] = random.nextInt(240);
			}
			int count = BarChart.runs(values, 240, HEIGHT, COLOUR, out);
			assertTrue(count >= 1 && count <= SAMPLES, "count=" + count);
		}
	}

	@Test
	@DisplayName("the colour follows the thresholds of the graph")
	void theColourFollowsTheThresholds() {
		int[] values = new int[SAMPLES];
		for (int index = 0; index < 18; index++) {
			values[index] = 30;
		}
		for (int index = 18; index < 36; index++) {
			values[index] = 60;
		}
		for (int index = 36; index < SAMPLES; index++) {
			values[index] = 120;
		}

		int[] out = buffer();
		int count = BarChart.runs(values, 120, HEIGHT, COLOUR, out);

		assertEquals(3, count);
		assertEquals(COLOUR.applyAsInt(30), out[3]);
		assertEquals(COLOUR.applyAsInt(60), out[7]);
		assertEquals(COLOUR.applyAsInt(120), out[11]);
	}
}

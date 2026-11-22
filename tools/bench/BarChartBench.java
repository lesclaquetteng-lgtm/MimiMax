import dev.aether.client.util.BarChart;

import java.util.Random;

/** Measures the grouping of the FPS graph bars: cost per chart and number of rectangles. */
public class BarChartBench {
	private static final int SAMPLES = 54;
	private static final int HEIGHT = 15;
	private static final int COLOUR_FPS = 0xFFF0B429;

	private static int colour(int fps) {
		return fps >= 120 ? 0xFF2ECC8F : fps >= 60 ? COLOUR_FPS : 0xFFE05260;
	}

	private static int rectangles(int[] values, int peak) {
		int[] out = new int[SAMPLES * 4];
		int count = BarChart.runs(values, peak, HEIGHT, BarChartBench::colour, out);
		int drawn = 0;
		for (int run = 0; run < count; run++) {
			if (out[run * 4 + 2] > 0) {
				drawn++;
			}
		}
		return drawn;
	}

	public static void main(String[] args) {
		int[] steady = new int[SAMPLES];
		java.util.Arrays.fill(steady, 60);

		int[] jitter = new int[SAMPLES];
		Random random = new Random(1L);
		for (int index = 0; index < SAMPLES; index++) {
			jitter[index] = 58 + random.nextInt(5);
		}

		int[] sawtooth = new int[SAMPLES];
		for (int index = 0; index < SAMPLES; index++) {
			sawtooth[index] = 30 + (index % 6) * 15;
		}

		for (String name : new String[] { "stable 60 fps", "jitter 58-62", "sawtooth 30-105" }) {
			int[] values = name.startsWith("stable") ? steady : name.startsWith("jitter") ? jitter : sawtooth;
			int peak = 120;
			System.out.printf("%-16s rectangles: old 54 -> new %d%n", name, rectangles(values, peak));
		}

		// The cost of the grouping itself, warmed up then measured over a million charts.
		int[] out = new int[SAMPLES * 4];
		for (int index = 0; index < 200_000; index++) {
			BarChart.runs(jitter, 120, HEIGHT, BarChartBench::colour, out);
		}
		int rounds = 2_000_000;
		long start = System.nanoTime();
		for (int index = 0; index < rounds; index++) {
			BarChart.runs(jitter, 120, HEIGHT, BarChartBench::colour, out);
		}
		long elapsed = System.nanoTime() - start;
		System.out.printf("grouping: %.1f ns per chart (%.3f us), %d charts%n",
				elapsed / (double) rounds, elapsed / 1000.0 / rounds, rounds);
	}
}

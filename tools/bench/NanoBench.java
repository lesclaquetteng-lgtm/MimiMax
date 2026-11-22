public class NanoBench {
	public static void main(String[] args) {
		int n = 200000;
		long start = System.nanoTime();
		for (int i = 0; i < n; i++) {
			System.nanoTime();
		}
		long elapsed = System.nanoTime() - start;
		System.out.println("nanoTime per call: " + (elapsed / (double) n) + " ns");
	}
}

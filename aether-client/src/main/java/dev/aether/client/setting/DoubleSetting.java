package dev.aether.client.setting;

/**
 * A setting holding a floating point number inside a range.
 */
public class DoubleSetting extends Setting {
	private final double defaultValue;
	private final double min;
	private final double max;
	private double value;

	public DoubleSetting(String name, String description, double defaultValue, double min, double max) {
		super(name, description);
		this.defaultValue = defaultValue;
		this.min = min;
		this.max = max;
		this.value = defaultValue;
	}

	@Override
	public Object getValue() {
		return value;
	}

	@Override
	public void setValue(Object value) {
		if (value instanceof Number number) {
			this.value = Math.max(min, Math.min(max, number.doubleValue()));
		} else if (value instanceof String stringValue) {
			try {
				this.value = Math.max(min, Math.min(max, Double.parseDouble(stringValue)));
			} catch (NumberFormatException ignored) {
				// keep the current value
			}
		}
	}

	@Override
	public void reset() {
		value = defaultValue;
	}

	@Override
	public String format() {
		return String.format(java.util.Locale.ROOT, "%.2f", value);
	}

	public double getMin() {
		return min;
	}

	/**
	 * @return the current value.
	 */
	public double get() {
		return value;
	}

	public double getMax() {
		return max;
	}
}

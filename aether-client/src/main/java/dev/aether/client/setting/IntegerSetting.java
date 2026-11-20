package dev.aether.client.setting;

/**
 * A setting holding a whole number inside a range.
 */
public class IntegerSetting extends Setting {
	private final int defaultValue;
	private final int min;
	private final int max;
	private int value;

	public IntegerSetting(String name, String description, int defaultValue, int min, int max) {
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
			this.value = Math.max(min, Math.min(max, number.intValue()));
		} else if (value instanceof String stringValue) {
			try {
				this.value = Math.max(min, Math.min(max, Integer.parseInt(stringValue)));
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
		return String.valueOf(value);
	}

	public int getMin() {
		return min;
	}

	/**
	 * @return the current value.
	 */
	public int get() {
		return value;
	}

	public int getMax() {
		return max;
	}

}

package dev.aether.client.setting;

import dev.aether.client.util.ColorUtil;

/**
 * A setting holding a colour.
 */
public class ColorSetting extends Setting {
	private final int defaultValue;
	private int value;

	public ColorSetting(String name, String description, int defaultValue) {
		super(name, description);
		this.defaultValue = defaultValue;
		this.value = defaultValue;
	}

	@Override
	public Object getValue() {
		return value;
	}

	@Override
	public void setValue(Object value) {
		if (value instanceof Number number) {
			this.value = number.intValue();
		} else if (value instanceof String stringValue) {
			this.value = ColorUtil.fromHex(stringValue, defaultValue);
		}
	}

	@Override
	public void reset() {
		value = defaultValue;
	}

	@Override
	public String format() {
		return ColorUtil.toHex(value);
	}

	public int getColor() {
		return value;
	}
}

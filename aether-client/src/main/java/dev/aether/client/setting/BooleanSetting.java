package dev.aether.client.setting;

/**
 * A setting that is either enabled or disabled.
 */
public class BooleanSetting extends Setting {
	private final boolean defaultValue;
	private boolean value;

	public BooleanSetting(String name, String description, boolean defaultValue) {
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
		if (value instanceof Boolean booleanValue) {
			this.value = booleanValue;
		} else if (value instanceof String stringValue) {
			this.value = Boolean.parseBoolean(stringValue);
		}
	}

	@Override
	public void reset() {
		value = defaultValue;
	}

	@Override
	public String format() {
		return value ? "ON" : "OFF";
	}

	public boolean isEnabled() {
		return value;
	}

	public void toggle() {
		value = !value;
	}
}

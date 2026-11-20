package dev.aether.client.setting;

import java.util.List;

/**
 * A setting that lets the user pick one of a fixed list of options.
 */
public class ModeSetting extends Setting {
	private final List<String> options;
	private final String defaultValue;
	private String value;

	public ModeSetting(String name, String description, String defaultValue, String... options) {
		super(name, description);
		this.options = List.of(options);
		this.defaultValue = defaultValue;
		this.value = defaultValue;
	}

	@Override
	public Object getValue() {
		return value;
	}

	@Override
	public void setValue(Object value) {
		if (value instanceof String stringValue && options.contains(stringValue)) {
			this.value = stringValue;
		}
	}

	@Override
	public void reset() {
		value = defaultValue;
	}

	@Override
	public String format() {
		return value;
	}

	@Override
	public List<?> getOptions() {
		return options;
	}

	/**
	 * @return the currently selected option.
	 */
	public String get() {
		return value;
	}

	/**
	 * Selects the next option in the list.
	 */
	public void cycle() {		int index = options.indexOf(value);
		value = options.get((index + 1) % options.size());
	}

	/**
	 * Selects the previous option in the list.
	 */
	public void cycleBack() {
		int index = options.indexOf(value);
		value = options.get((index - 1 + options.size()) % options.size());
	}
}

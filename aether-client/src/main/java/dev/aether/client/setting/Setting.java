package dev.aether.client.setting;

import java.util.List;

/**
 * A single configurable option of a module.
 */
public abstract class Setting {
	private final String name;
	private final String description;

	protected Setting(String name, String description) {
		this.name = name;
		this.description = description == null ? "" : description;
	}

	public String getName() {
		return name;
	}

	public String getDescription() {
		return description;
	}

	/**
	 * @return the current value.
	 */
	public abstract Object getValue();

	/**
	 * Applies a new value.
	 */
	public abstract void setValue(Object value);

	/**
	 * Restores the value the setting had when it was created.
	 */
	public abstract void reset();

	/**
	 * @return the value formatted for the interface.
	 */
	public abstract String format();

	/**
	 * @return every value the setting accepts, used by the mode and the colour settings.
	 */
	public List<?> getOptions() {
		return List.of();
	}

	/**
	 * @return the type of the value, used when the configuration is written.
	 */
	public String getType() {
		return getClass().getSimpleName();
	}
}

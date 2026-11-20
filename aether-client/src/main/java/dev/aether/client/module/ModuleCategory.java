package dev.aether.client.module;

/**
 * The categories the modules are grouped in inside the click GUI.
 */
public enum ModuleCategory {
	RENDER("Render"),
	HUD("HUD"),
	COMBAT("Combat"),
	MOVEMENT("Movement"),
	MISC("Misc");

	private final String displayName;

	ModuleCategory(String displayName) {
		this.displayName = displayName;
	}

	public String getDisplayName() {
		return displayName;
	}
}

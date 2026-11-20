package dev.aether.client.module;

import java.util.List;

/**
 * The registration keys the mixins look modules up by.
 *
 * <p>A mixin asks the module manager for a module by its key, while the module is registered under
 * the lower case form of its name. Both sides used to spell that string out, so renaming a module —
 * or its key — in one place left the other one looking for a module that no longer existed, and the
 * hook silently stopped doing anything. Both sides now read the same constant, and
 * {@code ModuleCatalogTest} fails when a key is not registered.</p>
 */
public final class ModuleKeys {
	public static final String ANTI_KNOCKBACK = "anti knockback";
	public static final String CROSSHAIR = "crosshair";
	public static final String HIDE_SCOREBOARD = "hide scoreboard";
	public static final String HIDE_FIRE_OVERLAY = "hide fire overlay";
	public static final String HIDE_HURT_OVERLAY = "hide hurt overlay";
	public static final String ZOOM = "zoom";

	/** Every key a mixin reads; each one has to exist in the catalogue. */
	public static final List<String> READ_BY_MIXINS = List.of(
			ANTI_KNOCKBACK,
			CROSSHAIR,
			HIDE_SCOREBOARD,
			HIDE_FIRE_OVERLAY,
			HIDE_HURT_OVERLAY,
			ZOOM);

	private ModuleKeys() {
	}
}

package dev.aether.client.module;

import com.mojang.blaze3d.platform.InputConstants;
import dev.aether.client.setting.Setting;
import dev.aether.client.util.Animation;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.resources.Identifier;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The base class of every feature of the client.
 *
 * A module can be enabled and disabled, owns a keybind and a list of settings and receives the
 * lifecycle callbacks of the client.
 */
public abstract class Module {
	private static final KeyMapping.Category CATEGORY =
			KeyMapping.Category.register(Identifier.fromNamespaceAndPath("aether", "modules"));

	private final String name;
	private final String description;
	private final ModuleCategory category;
	private final List<Setting> settings = new ArrayList<>();
	private final Animation toggleAnimation = new Animation(0.0f, 16.0f);

	private boolean enabled;
	private boolean defaultEnabled;
	private KeyMapping keyBinding;

	protected Module(String name, String description, ModuleCategory category, boolean defaultEnabled) {
		this.name = name;
		this.description = description;
		this.category = category;
		this.defaultEnabled = defaultEnabled;
		this.enabled = defaultEnabled;
		// The switch of the click GUI reads the animation, not the flag: a module enabled by
		// default must start with its switch on, otherwise setEnabled(true) is a no-op that leaves
		// the animation at zero and the row looks disabled while the module runs.
		this.toggleAnimation.set(defaultEnabled ? 1.0f : 0.0f);
	}

	/**
	 * Registers a setting and returns it so that it can be configured inline.
	 */
	protected <T extends Setting> T register(T setting) {
		settings.add(setting);
		return setting;
	}

	public String getName() {
		return name;
	}

	public String getDescription() {
		return description;
	}

	public ModuleCategory getCategory() {
		return category;
	}

	public List<Setting> getSettings() {
		return Collections.unmodifiableList(settings);
	}

	/**
	 * @return the setting with the given name, or null.
	 */
	public Setting getSetting(String settingName) {
		for (Setting setting : settings) {
			if (setting.getName().equalsIgnoreCase(settingName)) {
				return setting;
			}
		}
		return null;
	}

	public boolean isEnabled() {
		return enabled;
	}

	/**
	 * Enables or disables the module and runs the matching lifecycle callback.
	 */
	public void toggle() {
		setEnabled(!enabled);
	}

	public void setEnabled(boolean value) {
		if (this.enabled == value) {
			return;
		}
		this.enabled = value;
		toggleAnimation.setTarget(value ? 1.0f : 0.0f);
		if (value) {
			onEnable();
		} else {
			onDisable();
		}
	}

	/**
	 * Restores the module to the state it had when it was created.
	 *
	 * <p>The enabled state goes through {@link #setEnabled}, so the lifecycle callbacks run: resetting
	 * a configuration used to flip the flag only, which left the options a module had written
	 * (gamma, field of view, view bobbing, auto jump) at the value the module wanted while the module
	 * itself was off.</p>
	 */
	public void reset() {
		setEnabled(defaultEnabled);
		// The switch snaps back instead of animating: a reset is not a toggle by the player.
		toggleAnimation.set(enabled ? 1.0f : 0.0f);
		for (Setting setting : settings) {
			setting.reset();
		}
	}

	public float getToggleProgress() {
		return toggleAnimation.getValue();
	}

	public void updateAnimations(long deltaMillis) {
		toggleAnimation.update(deltaMillis);
	}

	public KeyMapping getKeyBinding() {
		return keyBinding;
	}

	/**
	 * Assigns a keybind to the module.
	 */
	public void setKeyBinding(KeyMapping keyBinding) {
		this.keyBinding = keyBinding;
	}

	/**
	 * Creates the keybind of the module.
	 *
	 * <p>The mapping is registered with Fabric: the loader is what turns a key press into a click on
	 * the mapping, so an unregistered one shows up in the controls screen and never fires. This has
	 * to happen while the client starts — Fabric throws once the options of the game exist — which
	 * is why every module receives its mapping at startup and the interface only moves the key.</p>
	 */
	public KeyMapping createKeyBinding(int keyCode, InputConstants.Type type) {
		KeyMapping mapping = new KeyMapping(
				"key.aether." + name.toLowerCase(java.util.Locale.ROOT).replace(' ', '_'),
				type,
				keyCode,
				CATEGORY
		);
		KeyBindingHelper.registerKeyBinding(mapping);
		setKeyBinding(mapping);
		return mapping;
	}

	/**
	 * Called when the module is enabled.
	 */
	public void onEnable() {
	}

	/**
	 * Called when the module is disabled.
	 */
	public void onDisable() {
	}

	/**
	 * Called every client tick.
	 */
	public void onTick() {
	}

	/**
	 * Called while the HUD is rendered.
	 */
	public void onRender(GuiGraphics graphics, float partialTick) {
	}
}

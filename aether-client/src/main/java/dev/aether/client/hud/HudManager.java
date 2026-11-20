package dev.aether.client.hud;

import dev.aether.client.config.AetherConfig;
import dev.aether.client.module.HudElement;
import net.minecraft.client.gui.GuiGraphics;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Keeps track of the HUD elements and draws them on the screen.
 */
public final class HudManager {
	private final List<HudElement> elements = new ArrayList<>();
	private int frames;

	/**
	 * Registers a HUD element.
	 */
	public void register(HudElement element) {
		elements.add(element);
	}

	/**
	 * Draws every enabled HUD element.
	 */
	public void render(GuiGraphics graphics, float partialTick) {
		// Measuring formats a few strings per element. Once every five frames is invisible to the
		// player and keeps the HUD out of the hot path of the frame.
		boolean measure = frames++ % 5 == 0;

		for (HudElement element : elements) {
			if (!element.isEnabled()) {
				continue;
			}
			if (measure) {
				element.updateSize();
			}
			element.render(graphics, partialTick);
		}
	}

	/**
	 * Places the elements that were never moved at a sensible position.
	 */
	public void applyDefaultPositions(int screenWidth, int screenHeight) {
		Map<String, int[]> defaults = new HashMap<>();
		defaults.put("fps", new int[] { 4, 4 });
		// The graph sits under the watermark, which is drawn at the same right edge.
		defaults.put("fps graph", new int[] { screenWidth - 124, 22 });
		defaults.put("ping", new int[] { 4, 20 });
		defaults.put("cps", new int[] { 4, 36 });
		defaults.put("coordinates", new int[] { 4, screenHeight - 26 });
		defaults.put("server brand", new int[] { 4, screenHeight - 42 });
		defaults.put("held item", new int[] { 4, screenHeight - 58 });
		defaults.put("session timer", new int[] { 4, screenHeight - 74 });
		defaults.put("session", new int[] { screenWidth - 124, screenHeight - 34 });
		defaults.put("keystrokes", new int[] { 4, screenHeight - 128 });
		defaults.put("armor", new int[] { 4, screenHeight - 100 });
		defaults.put("potions", new int[] { 4, screenHeight - 150 });
		defaults.put("target hud", new int[] { screenWidth / 2 - 60, 4 });

		for (HudElement element : elements) {
			String key = element.getName().toLowerCase(Locale.ROOT);
			int[] position = defaults.get(key);
			if (position != null) {
				element.setPosition(position[0], position[1]);
			}
		}
	}

	/**
	 * Restores the positions that were saved in the configuration.
	 */
	public void applySavedPositions(AetherConfig config) {
		for (HudElement element : elements) {
			String key = element.getName().toLowerCase(Locale.ROOT);
			int[] position = config.hudPositions.get(key);
			if (position != null && position.length >= 2) {
				element.setPosition(position[0], position[1]);
			}
		}
	}

	/**
	 * Stores the current positions in the configuration.
	 */
	public void capturePositions(AetherConfig config) {
		for (HudElement element : elements) {
			String key = element.getName().toLowerCase(Locale.ROOT);
			config.hudPositions.put(key, new int[] { element.getX(), element.getY() });
		}
	}

}

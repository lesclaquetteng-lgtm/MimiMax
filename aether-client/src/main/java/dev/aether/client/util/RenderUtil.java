package dev.aether.client.util;

import dev.aether.client.Aether;
import dev.aether.client.design.AetherDesignSystem;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.Minecraft;

/**
 * Rendering helpers shared by the HUD and the graphical interface.
 */
public final class RenderUtil {
	/*
	 * The graphite palette, kept in step with `launcher/gui/src/ui/tokens.css`: the launcher and the
	 * game must not look like two clients. A value lives here only when the renderer needs it as an
	 * int; the names match the CSS tokens so a change is easy to follow on both sides.
	 */
	public static final int PANEL_BACKGROUND = 0xFF101216;
	public static final int PANEL_BORDER = 0xFF21252B;
	public static final int TEXT_PRIMARY = 0xFFF2F4F6;
	public static final int TEXT_MUTED = 0xFFA9B1B9;
	public static final int TEXT_DIM = 0xFF767F88;

	/** The surfaces of the interface, from the panel to the row the mouse is over. */
	public static final int SURFACE = 0xFF121418;
	public static final int SURFACE_RAISED = 0xFF171A1F;
	public static final int SURFACE_HOVER = 0xFF191C21;
	public static final int SURFACE_ACTIVE = 0xFF1C2026;
	public static final int INNER = 0xFF0A0C0F;
	public static final int BORDER = 0xFF21252B;
	public static final int BORDER_STRONG = 0xFF2C323A;
	public static final int SHADOW = 0x55000000;

	private static volatile AetherDesignSystem.Palette cachedTheme=AetherDesignSystem.Colorway.GRAPHITE.palette();
	private static volatile String cachedColorway;
	private static volatile String cachedSurfaceTreatment;
	private static volatile String cachedAccentColor;
	private static volatile boolean cachedCustomAccent;

	private RenderUtil() {}

	/** Current immutable palette, refreshed only when a stored appearance preference changes. */
	public static AetherDesignSystem.Palette theme(){
		Aether aether=Aether.getOrNull();
		if(aether==null)return AetherDesignSystem.Colorway.GRAPHITE.palette();
		var config=aether.getConfig();
		if(!same(config.colorway,cachedColorway)
				||!same(config.surfaceTreatment,cachedSurfaceTreatment)
				||config.customAccent!=cachedCustomAccent
				||!same(config.accentColor,cachedAccentColor)){
			synchronized(RenderUtil.class){
				if(!same(config.colorway,cachedColorway)
						||!same(config.surfaceTreatment,cachedSurfaceTreatment)
						||config.customAccent!=cachedCustomAccent
						||!same(config.accentColor,cachedAccentColor)){
					cachedTheme=AetherDesignSystem.resolve(config);
					cachedColorway=config.colorway;
					cachedSurfaceTreatment=config.surfaceTreatment;
					cachedCustomAccent=config.customAccent;
					cachedAccentColor=config.accentColor;
				}
			}
		}
		return cachedTheme;
	}

	private static boolean same(String left,String right){
		return left==right||(left!=null&&left.equals(right));
	}

	/**
	 * The chords of every small radius, computed once.
	 *
	 * <p>A rounded rectangle is drawn column by column and each column needs the chord of the arc.
	 * The chord only depends on the radius, so the square roots are paid once per radius instead of
	 * once per column per call: the HUD alone draws dozens of rounded rectangles per frame.</p>
	 */
	private static final int[][] CHORD_CACHE = new int[65][];

	/**
	 * @return the chord of each column of a corner of the given radius, from the outermost column
	 *         to the centre. Exposed for the tests.
	 */
	static int[] chords(int radius) {
		if (radius <= 0) {
			return new int[0];
		}
		if (radius < CHORD_CACHE.length) {
			int[] cached = CHORD_CACHE[radius];
			if (cached == null) {
				cached = computeChords(radius);
				CHORD_CACHE[radius] = cached;
			}
			return cached;
		}
		return computeChords(radius);
	}

	private static int[] computeChords(int radius) {
		int[] chords = new int[radius];
		for (int i = 0; i < radius; i++) {
			double distance = radius - i - 0.5;
			chords[i] = (int) Math.round(
					Math.sqrt(Math.max(0.0, radius * radius - distance * distance)));
		}
		return chords;
	}

	/**
	 * Draws a rectangle with rounded corners.
	 *
	 * The corners are drawn column by column, which produces a smooth result without needing a
	 * texture or a shader. The chords come from {@link #chords(int)}, so the hot path only fills
	 * rectangles.
	 */
	public static void roundedRect(GuiGraphics graphics, int x, int y, int width, int height, int radius, int color) {
		if (width <= 0 || height <= 0) {
			return;
		}

		int effectiveRadius = Math.min(radius, Math.min(width, height) / 2);
		if (effectiveRadius <= 0) {
			graphics.fill(x, y, x + width, y + height, color);
			return;
		}

		// The straight parts of the rectangle.
		graphics.fill(x + effectiveRadius, y, x + width - effectiveRadius, y + height, color);
		graphics.fill(x, y + effectiveRadius, x + width, y + height - effectiveRadius, color);

		// The corners, column by column. Each column is filled from the arc of the top corner to
		// the arc of the bottom one, so a pill (a radius of half the height) stays solid instead
		// of leaving a hole in the middle of its ends.
		int[] chords = chords(effectiveRadius);
		for (int i = 0; i < effectiveRadius; i++) {
			int chord = chords[i];
			int top = y + effectiveRadius - chord;
			int bottom = y + height - effectiveRadius + chord;

			graphics.fill(x + i, top, x + i + 1, bottom, color);
			graphics.fill(x + width - i - 1, top, x + width - i, bottom, color);
		}
	}

	/**
	 * Draws a soft shadow under a rounded rectangle.
	 *
	 * Three translucent layers are enough to read as depth without a texture or a blur pass.
	 */
	public static void shadow(GuiGraphics graphics, int x, int y, int width, int height, int radius) {
		// One pass instead of three stacked layers: each layer cost a full rounded rectangle, and
		// the halo they added was almost invisible once blended. The measurement in
		// ANALYSE-AMELIORATIONS.md counts what this saves per frame.
		int spread = 4;
		roundedRect(graphics, x - spread, y - spread + 2, width + spread * 2, height + spread * 2,
				radius + spread, 0x30000000);
	}

	/**
	 * Draws a card: a soft shadow, the surface and a one pixel border with specular highlight.
	 */
	public static void card(GuiGraphics graphics, int x, int y, int width, int height, int radius) {
		card(graphics, x, y, width, height, radius, 1.0f);
	}

	/**
	 * Draws a card with an opacity, used by the elements that fade in and out.
	 *
	 * <p>The opacity multiplies the alpha of every layer instead of replacing it, so the translucent
	 * surfaces keep their designed weight at full opacity.</p>
	 */
	public static void card(GuiGraphics graphics, int x, int y, int width, int height, int radius,
			float opacity) {
		if (width <= 0 || height <= 0 || opacity <= 0.004f) {
			return;
		}
		int spread = 4;
		roundedRect(graphics, x - spread, y - spread + 2, width + spread * 2, height + spread * 2,
				radius + spread, ColorUtil.fade(0x30000000, opacity));
		AetherDesignSystem.Palette palette=theme();
		bordered(graphics,x,y,width,height,radius,
				ColorUtil.fade(palette.surface(),opacity),ColorUtil.fade(palette.border(),opacity));
		if (width > radius * 2 && height > 2) {
			graphics.fill(x + radius, y + 1, x + width - radius, y + 2, ColorUtil.fade(0x1AFFFFFF, opacity));
		}
	}

	/**
	 * Draws a rounded rectangle with a one pixel border.
	 *
	 * <p>The border is painted as a full rectangle under the fill, so the fill must be opaque: a
	 * translucent or transparent fill leaves the border colour showing through. Use {@link #outline}
	 * whenever the inside has to stay visible.</p>
	 */
	public static void bordered(GuiGraphics graphics, int x, int y, int width, int height, int radius,
			int fill, int border) {
		roundedRect(graphics, x, y, width, height, radius, border);
		roundedRect(graphics, x + 1, y + 1, width - 2, height - 2, Math.max(0, radius - 1), fill);
	}

	/**
	 * Draws a one pixel rectangular outline, leaving everything inside untouched.
	 */
	public static void outline(GuiGraphics graphics, int x, int y, int width, int height, int colour) {
		if (width <= 0 || height <= 0) {
			return;
		}
		graphics.fill(x, y, x + width, y + 1, colour);
		graphics.fill(x, y + height - 1, x + width, y + height, colour);
		graphics.fill(x, y + 1, x + 1, y + height - 1, colour);
		graphics.fill(x + width - 1, y + 1, x + width, y + height - 1, colour);
	}

	/**
	 * Draws a rounded progress bar.
	 *
	 * @param progress the filled part, clamped between 0 and 1
	 */
	public static void bar(GuiGraphics graphics, int x, int y, int width, int height, float progress,
			int track, int fill) {
		if (width <= 0 || height <= 0) {
			return;
		}

		int radius = Math.max(1, height / 2);
		roundedRect(graphics, x, y, width, height, radius, track);

		int filled = Math.round(width * Math.max(0.0f, Math.min(1.0f, progress)));
		if (filled > 1) {
			roundedRect(graphics, x, y, filled, height, radius, fill);
		}
	}

	/**
	 * Draws the handle of a scrollbar.
	 */
	public static void scrollbar(GuiGraphics graphics, int x, int y, int height, float offset,
			float contentHeight, int accent) {
		if (contentHeight <= 0) {
			return;
		}

		float visible = Math.min(1.0f, height / contentHeight);
		int thumb = Math.max(18, Math.round(height * visible));
		int travel = height - thumb;
		float maxOffset = contentHeight - height;
		int position = maxOffset <= 0 ? 0 : Math.round(travel * (offset / maxOffset));

		roundedRect(graphics, x, y, 3, height, 1, AetherDesignSystem.scrollTrack(theme()));
		roundedRect(graphics, x, y + position, 3, thumb, 1, AetherDesignSystem.scrollThumb(accent));
	}

	/**
	 * The vertical distance an overlay still has to travel when it opens. The decoration uses it
	 * for a fade-and-lift entrance; interactive hitboxes stay where the layout put them.
	 */
	public static int enterLift(long openedAt) {
		float progress = Math.min(1.0f,
				Math.max(0.0f, (System.nanoTime() - openedAt) / (float) AetherDesignSystem.Metrics.ENTER_NANOS));
		float remaining = 1.0f - progress;
		return Math.round(remaining * remaining * remaining * AetherDesignSystem.Metrics.ENTER_LIFT);
	}

	/**
	 * Paints the Aurora Nightfall backdrop: a night gradient, two drifting aurora curtains, a
	 * scatter of stars, a pixel moon and a voxel horizon. It is deliberately cheap: the curtains
	 * are short runs of rectangles, not a texture or a shader.
	 */
	public static void auroraBackdrop(GuiGraphics graphics, int width, int height, long now,
			float opacity) {
		if (width <= 0 || height <= 0 || opacity <= 0.004f) {
			return;
		}
		AetherDesignSystem.Palette palette = theme();
		int night = ColorUtil.lerp(palette.background(), AetherDesignSystem.Scene.MOUNTAIN, 0.42f);
		graphics.fill(0, 0, width, height, ColorUtil.fade(night, opacity));

		int drift = (int) ((now / 1_000_000L) % 20_000L);
		auroraCurtain(graphics, width, Math.round(height * 0.17f), Math.max(8, height / 28),
				26, AetherDesignSystem.Scene.AURORA_GREEN, 0.12f, drift, opacity);
		auroraCurtain(graphics, width, Math.round(height * 0.29f), Math.max(10, height / 22),
				18, AetherDesignSystem.Scene.AURORA_TEAL, 0.09f, drift + 1_300, opacity);
		auroraCurtain(graphics, width, Math.round(height * 0.40f), Math.max(6, height / 34),
				10, AetherDesignSystem.Scene.AURORA_VIOLET, 0.05f, drift + 2_700, opacity);

		for (int index = 0; index < 44; index++) {
			int x = Math.floorMod(noise(index * 37 + 11), Math.max(1, width));
			int y = Math.floorMod(noise(index * 91 + 7), Math.max(1, (int) (height * 0.68f)));
			int size = (noise(index * 17 + 3) & 3) == 0 ? 2 : 1;
			float twinkle = 0.30f + 0.55f
					* (0.5f + 0.5f * (float) Math.sin(now / 780_000_000.0 + index * 0.71));
			graphics.fill(x, y, x + size, y + size,
					ColorUtil.fade(ColorUtil.withAlpha(AetherDesignSystem.Scene.STAR, twinkle), opacity));
		}

		pixelMoon(graphics, width - 66, 28, now, opacity);
		voxelHorizon(graphics, width, height, opacity);
	}

	private static void auroraCurtain(GuiGraphics graphics, int width, int centerY, int amplitude,
			int thickness, int colour, float alpha, int drift, float opacity) {
		int paint = ColorUtil.fade(ColorUtil.withAlpha(colour, alpha), opacity);
		int tail = ColorUtil.fade(ColorUtil.withAlpha(colour, alpha * 0.35f), opacity);
		for (int x = 0; x < width; x += 24) {
			double wave = Math.sin((x + drift * 0.14) * 0.018)
					+ 0.45 * Math.sin((x - drift * 0.09) * 0.041);
			int y = centerY + Math.round((float) wave * amplitude);
			graphics.fill(x, y, x + 26, y + thickness, paint);
			graphics.fill(x, y + thickness, x + 26, y + thickness + 8, tail);
		}
	}

	private static void pixelMoon(GuiGraphics graphics, int x, int y, long now, float opacity) {
		int glow = ColorUtil.fade(ColorUtil.withAlpha(AetherDesignSystem.Scene.MOON, 0.06f), opacity);
		roundedRect(graphics, x - 12, y - 12, 42, 42, 21, glow);
		roundedRect(graphics, x - 5, y - 5, 28, 28, 14,
				ColorUtil.fade(ColorUtil.withAlpha(AetherDesignSystem.Scene.MOON, 0.09f), opacity));

		int moon = ColorUtil.fade(ColorUtil.withAlpha(AetherDesignSystem.Scene.MOON, 0.82f), opacity);
		int crater = ColorUtil.fade(ColorUtil.withAlpha(AetherDesignSystem.Scene.MOON, 0.36f), opacity);
		graphics.fill(x, y + 4, x + 18, y + 14, moon);
		graphics.fill(x + 4, y, x + 14, y + 18, moon);
		graphics.fill(x + 3, y + 3, x + 6, y + 6, crater);
		graphics.fill(x + 11, y + 8, x + 14, y + 11, crater);
		graphics.fill(x + 6, y + 12, x + 8, y + 14, crater);

		int pulse = 1 + Math.round((0.5f + 0.5f * (float) Math.sin(now / 1_400_000_000.0)) * 3.0f);
		outline(graphics, x - pulse, y - pulse, 18 + pulse * 2, 18 + pulse * 2,
				ColorUtil.fade(ColorUtil.withAlpha(AetherDesignSystem.Scene.STAR, 0.06f), opacity));
	}

	private static void voxelHorizon(GuiGraphics graphics, int width, int height, float opacity) {
		AetherDesignSystem.Palette palette = theme();
		for (int layer = 0; layer < 2; layer++) {
			int baseline = height - 10 + layer * 7;
			int colour = ColorUtil.fade(ColorUtil.lerp(AetherDesignSystem.Scene.MOUNTAIN,
					palette.background(), 0.08f + layer * 0.12f), opacity);
			int step = 28 + layer * 8;
			for (int x = -step; x < width + step; x += step) {
				int peak = 9 + Math.floorMod(noise(x + layer * 71), 25);
				graphics.fill(x, baseline - peak, x + step, height, colour);
				if ((noise(x + 19) & 3) == 0) {
					graphics.fill(x + 6, baseline - peak - 6, x + 14, baseline - peak, colour);
				}
			}
		}
	}

	private static int noise(int seed) {
		int value = seed * 0x1F123BB5;
		value ^= value >>> 15;
		value *= 0x2C1B3C6D;
		return value ^ (value >>> 12);
	}

	/**
	 * A frosted panel: a shadow, an accent halo, the glass surface, a luminous border and a
	 * specular line along the top edge.
	 */
	public static void glassPanel(GuiGraphics graphics, int x, int y, int width, int height,
			int radius, int accent, float opacity) {
		if (width <= 0 || height <= 0 || opacity <= 0.004f) {
			return;
		}
		AetherDesignSystem.Palette palette = theme();
		int spread = 4;
		roundedRect(graphics, x - spread, y - spread + 2, width + spread * 2, height + spread * 2,
				radius + spread, ColorUtil.fade(SHADOW, opacity));
		roundedRect(graphics, x - 5, y - 5, width + 10, height + 10, radius + 5,
				ColorUtil.fade(AetherDesignSystem.glow(accent, 0.08f), opacity));
		roundedRect(graphics, x - 2, y - 2, width + 4, height + 4, radius + 2,
				ColorUtil.fade(AetherDesignSystem.glow(accent, 0.13f), opacity));
		bordered(graphics, x, y, width, height, radius,
				ColorUtil.fade(AetherDesignSystem.glass(palette), opacity),
				ColorUtil.fade(AetherDesignSystem.glassBorder(palette, accent), opacity));
		if (width > radius * 2) {
			graphics.fill(x + radius, y + 1, x + width - radius, y + 2,
					ColorUtil.fade(AetherDesignSystem.glassHighlight(palette), opacity));
			graphics.fill(x + radius, y + 2, x + width - radius, y + 3,
					ColorUtil.fade(ColorUtil.withAlpha(palette.textPrimary(), 0.035f), opacity));
		}
	}

	/** A quieter glass surface for rails and columns inside a panel. */
	public static void glassInset(GuiGraphics graphics, int x, int y, int width, int height,
			int radius, int accent, float opacity) {
		if (width <= 0 || height <= 0 || opacity <= 0.004f) {
			return;
		}
		AetherDesignSystem.Palette palette = theme();
		bordered(graphics, x, y, width, height, radius,
				ColorUtil.fade(AetherDesignSystem.glassDeep(palette), opacity),
				ColorUtil.fade(AetherDesignSystem.glassBorder(palette), opacity));
		if (width > radius * 2) {
			graphics.fill(x + radius, y + 1, x + width - radius, y + 2,
					ColorUtil.fade(AetherDesignSystem.glassHighlight(palette), opacity * 0.55f));
		}
	}

	/** A module row, with the accent halo reserved for the enabled state. */
	public static void rowCard(GuiGraphics graphics, int x, int y, int width, int height, int radius,
			int accent, boolean active, boolean hovered, float opacity) {
		AetherDesignSystem.Palette palette = theme();
		if (active) {
			roundedRect(graphics, x - 2, y - 2, width + 4, height + 4, radius + 2,
					ColorUtil.fade(AetherDesignSystem.glow(accent, 0.09f), opacity));
		}
		bordered(graphics, x, y, width, height, radius,
				ColorUtil.fade(AetherDesignSystem.row(palette, accent, active, hovered), opacity),
				ColorUtil.fade(AetherDesignSystem.rowBorder(palette, accent, active, hovered), opacity));
		if (active) {
			roundedRect(graphics, x + 1, y + Math.max(4, height / 2 - (height - 24) / 2), 2,
					Math.min(height - 8, 24), 1, ColorUtil.fade(accent, opacity));
		}
		if (width > radius * 2) {
			graphics.fill(x + radius, y + 1, x + width - radius, y + 2,
					ColorUtil.fade(AetherDesignSystem.glassHighlight(palette), opacity * 0.45f));
		}
	}

	/** A slider track, its filled part and a small handle the click geometry also follows. */
	public static void slider(GuiGraphics graphics, int x, int y, int width, float progress,
			int track, int fill, int knob) {
		if (width <= 0) {
			return;
		}
		float ratio = Math.max(0.0f, Math.min(1.0f, progress));
		roundedRect(graphics, x, y, width, 3, 1, track);
		int filled = Math.round(width * ratio);
		if (filled > 1) {
			roundedRect(graphics, x, y, filled, 3, 1, fill);
		}
		int size = 5;
		int knobX = x + Math.round((width - size) * ratio);
		roundedRect(graphics, knobX - 1, y - 2, size + 2, size + 2, (size + 2) / 2,
				ColorUtil.withAlpha(knob, 0.20f));
		roundedRect(graphics, knobX, y - 1, size, size, size / 2, knob);
	}

	/** A compact state chip, used for counts and keybinds. */
	public static void chip(GuiGraphics graphics, int x, int y, int width, int height, int radius,
			int fill, int border) {
		bordered(graphics, x, y, width, height, radius, fill, border);
	}

	/**
	 * A category or count badge. The width is returned, so the caller can align the next element
	 * without measuring the label a second time.
	 */
	public static int badge(GuiGraphics graphics, Font font, String label, int x, int y, int minWidth,
			int accent, boolean active, boolean hovered, float opacity) {
		AetherDesignSystem.Palette palette = theme();
		int width = Math.max(minWidth, font.width(label) + 10);
		int height = 14;
		roundedRect(graphics, x, y, width, height, height / 2,
				ColorUtil.fade(AetherDesignSystem.badge(palette, accent, active, hovered), opacity));
		if (active) {
			roundedRect(graphics, x + 5, y + height - 1, width - 10, 1, 0,
					ColorUtil.fade(accent, opacity));
		}
		centeredText(graphics, font, label, x + width / 2, y + 3,
				ColorUtil.fade(AetherDesignSystem.badgeText(palette, accent, active, hovered), opacity));
		return width;
	}

	/**
	 * Draws text with a shadow.
	 */
	public static void text(GuiGraphics graphics, Font font, String value, int x, int y, int color) {
		graphics.drawString(font, value, x, y, color, false);
	}

	/**
	 * Draws text centred horizontally.
	 */
	public static void centeredText(GuiGraphics graphics, Font font, String value, int centerX, int y, int color) {
		graphics.drawString(font, value, centerX - font.width(value) / 2, y, color, false);
	}

	/**
	 * Draws text with an optional shadow.
	 */
	public static void text(GuiGraphics graphics, Font font, String value, int x, int y, int color, boolean shadow) {
		graphics.drawString(font, value, x, y, color, shadow);
	}

	/**
	 * @return the width of the given text.
	 */
	public static int width(Font font, String value) {
		return font.width(value);
	}

	/**
	 * @return the current screen width, or 0 when the window does not exist yet.
	 */
	public static int screenWidth() {
		var window = Minecraft.getInstance() == null ? null : Minecraft.getInstance().getWindow();
		return window == null ? 0 : window.getGuiScaledWidth();
	}

	/**
	 * @return the current screen height, or 0 when the window does not exist yet.
	 */
	public static int screenHeight() {
		var window = Minecraft.getInstance() == null ? null : Minecraft.getInstance().getWindow();
		return window == null ? 0 : window.getGuiScaledHeight();
	}

	/**
	 * Truncates a string so that it fits into the given width.
	 *
	 * <p>The work is done by {@link TextFit}, which finds the longest fitting prefix by dichotomy: a
	 * handful of measurements instead of one per removed character.</p>
	 */
	public static String truncate(Font font, String value, int maxWidth) {
		return TextFit.truncate(value, maxWidth, font::width);
	}
}

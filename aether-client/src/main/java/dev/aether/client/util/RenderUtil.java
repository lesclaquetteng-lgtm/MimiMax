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

		roundedRect(graphics,x,y,3,height,1,ColorUtil.withAlpha(theme().borderStrong(),0.55f));
		roundedRect(graphics,x,y+position,3,thumb,1,ColorUtil.withAlpha(accent,0.9f));
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

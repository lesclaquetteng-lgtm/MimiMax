package dev.aether.client.gui;

import com.mojang.blaze3d.platform.InputConstants;
import dev.aether.client.Aether;
import dev.aether.client.design.AetherDesignSystem;
import dev.aether.client.module.Module;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.util.SessionUtil;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

import java.util.ArrayList;
import java.util.List;

/**
 * The settings of the client, in the same language as the launcher.
 *
 * <p>One panel: a header row, a hairline, two columns separated by a hairline, a hairline and a
 * footer with the only primary action of the screen. The accent marks the selected swatch, the row
 * being rebound and the primary button; everything else is graphite. No stripe, no pill, no glyph
 * in front of a label, no card inside a card.</p>
 *
 * <p>The geometry is computed once by {@link #layout()} and read by both the renderer and the input
 * handlers: the previous version recomputed the same offsets by hand in six places, and a swatch
 * that moved on screen no longer answered the mouse.</p>
 */
public class ConfigScreen extends Screen {

	private static final int PANEL_W = 520;
	private static final int PANEL_H = 360;
	private static final int PAD = AetherDesignSystem.Metrics.SPACE_LG;
	private static final int PANEL_RADIUS = AetherDesignSystem.Metrics.RADIUS_XL;
	private static final int CARD_RADIUS = AetherDesignSystem.Metrics.RADIUS_MD;
	private static final int CONTROL_RADIUS = AetherDesignSystem.Metrics.RADIUS_SM;
	private static final int SMALL_RADIUS = AetherDesignSystem.Metrics.RADIUS_XS;
	private static final long ENTRANCE_NANOS = AetherDesignSystem.Metrics.ENTER_NANOS;
	private static final int HEADER_H = 38;
	private static final int FOOTER_H = 44;
	private static final int LEFT_W = 196;
	private static final int COL_GAP = 20;

	private static final int ROW_H = 24;
	private static final int SWATCH = 22;
	private static final int SWATCH_GAP = 4;
	private static final int SWATCH_COLUMNS = 4;
	private static final int BTN_H = 26;

	private int panelX;
	private int panelY;
	private int panelW = PANEL_W;
	private int panelH = PANEL_H;
	private int leftX;
	private int leftW = LEFT_W;
	private int rightX;
	private int rightW;
	private int bodyTop;
	private int bodyBottom;
	private int colorwayTop;
	private int surfaceTop;
	private int accentTop;
	private int swatchSize = SWATCH;
	private int swatchColumns = SWATCH_COLUMNS;
	private int swatchGap = SWATCH_GAP;
	private boolean compact;

	private int listTop;
	private int listBottom;
	private int scroll;
	private int maxScroll;

	private final List<FooterButton> footer = new ArrayList<>();

	/** One rectangle of the footer, with the label it draws and whether it is the primary action. */
	private record FooterButton(int x, int y, int w, int h, String label, boolean primary) {
	}

	private Module rebinding;
	private List<Module> modules;
	private final long openedAt = System.nanoTime();
	private float opacity = 1.0f;

	public ConfigScreen() {
		super(Component.literal("Aether Configuration"));
	}

	// ══════════════════════════════════════════════════════════════════ lifecycle

	@Override
	protected void init() {
		modules = new ArrayList<>(Aether.get().getModuleManager().getAll());
		layout();
	}

	/**
	 * Computes every rectangle of the screen once. Rendering and input both read these fields.
	 */
	private void layout() {
		// The panel is a preferred size, not a fixed one: at the smallest supported window the GUI
		// is 450 units wide, and a 520 unit panel would be clipped by the screen it sits on.
		panelW = Math.min(PANEL_W, width - 24);
		panelH = Math.min(PANEL_H, height - 24);
		panelX = (width - panelW) / 2;
		panelY = (height - panelH) / 2;
		leftX = panelX + PAD;
		leftW = Math.max(150, Math.min(LEFT_W, panelW - PAD * 2 - 140));
		rightX = leftX + leftW + COL_GAP;
		rightW = panelX + panelW - PAD - rightX;
		bodyTop = panelY + HEADER_H + 16;
		bodyBottom = panelY + panelH - FOOTER_H - 14;

		// The session block and the grid of accents share the left column: on a short window the
		// sections fold into four compact rows instead of running under the footer. Everything stays
		// clickable; only the spacing and the colourway grid change.
		compact = panelH < 300;
		if (compact) {
			colorwayTop = bodyTop + 18;
			surfaceTop = colorwayTop + 30;
			accentTop = surfaceTop + 42;
			swatchColumns = 12;
			swatchGap = 3;
			swatchSize = Math.max(10, Math.min(14,
					(leftW - (swatchColumns - 1) * swatchGap) / swatchColumns));
		} else {
			colorwayTop = bodyTop + 62;
			surfaceTop = colorwayTop + 58;
			accentTop = surfaceTop + 58;
			int room = bodyBottom - accentTop;
			swatchColumns = SWATCH_COLUMNS;
			swatchGap = SWATCH_GAP;
			swatchSize = Math.max(12, Math.min(18,
					(room - (3 - 1) * swatchGap) / 3));
		}

		listTop = bodyTop + 18;
		listBottom = bodyBottom;
		// The list is scrolled by the wheel and mapped back to rows by the click handler: the
		// clamp belongs here, so a click made before the next frame cannot land on another row.
		maxScroll = Math.max(0, modules.size() * ROW_H - (listBottom - listTop));
		scroll = Math.max(0, Math.min(scroll, maxScroll));

		Font font = Minecraft.getInstance().font;
		footer.clear();
		int y = panelY + panelH - FOOTER_H + (FOOTER_H - BTN_H) / 2;
		int x = panelX + panelW - PAD;

		// Laid out from the right edge: the primary action holds the edge, the secondary ones queue
		// behind it, so adding a button never moves « Done » away from the corner.
		int doneW = font.width("Done") + 24;
		x -= doneW;
		footer.add(new FooterButton(x, y, doneW, BTN_H, "Done", true));
		x -= 8;
		for (String label : List.of("Reset settings", "Reset modules", "HUD editor")) {
			int w = font.width(label) + 20;
			x -= w;
			footer.add(0, new FooterButton(x, y, w, BTN_H, label, false));
			x -= 8;
		}
	}

	// ═══════════════════════════════════════════════════════════════════ rendering

		@Override
	public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {
		Minecraft minecraft = Minecraft.getInstance();
		Font font = minecraft.font;
		int accent = Aether.get().getConfig().getAccentColor();
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		opacity = entrance();

		RenderUtil.auroraBackdrop(graphics, width, height, System.nanoTime(), 0.96f * opacity);
		RenderUtil.glassPanel(graphics, panelX, panelY, panelW, panelH, PANEL_RADIUS, accent, opacity);
		renderHeader(graphics, font, accent);
		renderSession(graphics, font, accent);
		renderColorways(graphics, font, mouseX, mouseY, accent);
		renderSurfaceTreatments(graphics, font, mouseX, mouseY, accent);
		renderAccent(graphics, font, mouseX, mouseY, accent);
		renderModules(graphics, font, mouseX, mouseY, accent);
		renderFooter(graphics, font, mouseX, mouseY, accent);

		// The vertical hairline that makes the two columns two columns.
		graphics.fill(leftX + leftW + COL_GAP / 2, bodyTop, leftX + leftW + COL_GAP / 2 + 1, bodyBottom,
				ColorUtil.withAlpha(palette.textPrimary(), 0.10f * opacity));
	}

	private void renderHeader(GuiGraphics graphics, Font font, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		int lift = RenderUtil.enterLift(openedAt);
		RenderUtil.roundedRect(graphics, panelX + 24, panelY - 1 + lift, panelW - 48, 2, 1,
				ColorUtil.fade(AetherDesignSystem.glow(accent, 0.34f), opacity));
		RenderUtil.roundedRect(graphics, leftX, panelY + 12, 5, 5, 2, fade(accent));
		RenderUtil.text(graphics, font, "AETHER", leftX + 11, panelY + 11, tone(AetherDesignSystem.TextRole.HINT));
		RenderUtil.text(graphics, font, "Configuration", leftX, panelY + 22, tone(AetherDesignSystem.TextRole.TITLE));
		String version = "v1.0";
		int badgeWidth = font.width(version) + 10;
		RenderUtil.badge(graphics, font, version, panelX + panelW - PAD - badgeWidth, panelY + 11, 0,
				accent, false, false, opacity);
		hairline(graphics, panelY + HEADER_H);
	}

	private void hairline(GuiGraphics graphics, int y) {
		graphics.fill(leftX, y, panelX + panelW - PAD, y + 1,
				ColorUtil.withAlpha(RenderUtil.theme().textPrimary(), 0.08f * opacity));
	}

	/** Avatar, name and the one bit that matters about the session: offline or Microsoft. */
	private void renderSession(GuiGraphics graphics, Font font, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		String username = SessionUtil.getUsername();
		String initials = username.isEmpty() ? "?" : username.substring(0, 1).toUpperCase();

		if (compact) {
			int rowY = bodyTop;
			boolean offline = SessionUtil.isOffline();
			int dot = offline ? AetherDesignSystem.Scene.OFFLINE : AetherDesignSystem.Scene.ONLINE;
			int avatar = ColorUtil.lerp(
					AetherDesignSystem.surface(palette, AetherDesignSystem.SurfaceState.ACTIVE),
					accent, 0.26f);
			RenderUtil.roundedRect(graphics, leftX, rowY, 14, 14, SMALL_RADIUS,
					ColorUtil.fade(avatar, opacity));
			RenderUtil.text(graphics, font, initials, leftX + (14 - font.width(initials)) / 2,
					rowY + 3, fade(accent));
			String name = RenderUtil.truncate(font, username, leftW - 92);
			RenderUtil.text(graphics, font, name, leftX + 20, rowY + 1,
					tone(AetherDesignSystem.TextRole.TITLE));
			int statusX = leftX + 20 + font.width(name) + 8;
			graphics.fill(statusX, rowY + 5, statusX + 4, rowY + 9, fade(dot));
			RenderUtil.text(graphics, font, offline ? "Offline" : "Microsoft", statusX + 8,
					rowY + 1, tone(AetherDesignSystem.TextRole.HINT));
			return;
		}

		RenderUtil.text(graphics, font, "SESSION", leftX, bodyTop, tone(AetherDesignSystem.TextRole.HINT));

		int y = bodyTop + 14;

		int avatar = ColorUtil.lerp(AetherDesignSystem.surface(palette, AetherDesignSystem.SurfaceState.ACTIVE),
				accent, 0.26f);
		RenderUtil.roundedRect(graphics, leftX, y, 26, 26, CONTROL_RADIUS, fade(avatar));
		RenderUtil.outline(graphics, leftX, y, 26, 26,
				ColorUtil.withAlpha(accent, 0.30f * opacity));
		RenderUtil.text(graphics, font, initials, leftX + (26 - font.width(initials)) / 2, y + 9, fade(accent));

		RenderUtil.text(graphics, font, RenderUtil.truncate(font, username, leftW - 42), leftX + 34, y + 2,
				tone(AetherDesignSystem.TextRole.TITLE));

		boolean offline = SessionUtil.isOffline();
		int dot = offline ? AetherDesignSystem.Scene.OFFLINE : AetherDesignSystem.Scene.ONLINE;
		String state = offline ? "Offline" : "Microsoft";
		int pillWidth = font.width(state) + 18;
		RenderUtil.roundedRect(graphics, leftX + 34, y + 11, pillWidth, 14, 7,
				ColorUtil.withAlpha(dot, 0.12f * opacity));
		graphics.fill(leftX + 40, y + 16, leftX + 44, y + 20, fade(dot));
		RenderUtil.text(graphics, font, state, leftX + 48, y + 14, tone(AetherDesignSystem.TextRole.LABEL));

		hairline(graphics, y + 40);
	}

	private int[] colorwayRect(int index) {
		int columns = compact ? 4 : 2;
		int gap = compact ? AetherDesignSystem.Metrics.SPACE_XS : AetherDesignSystem.Metrics.SPACE_XS + 2;
		int cardWidth = (leftW - gap * (columns - 1)) / columns;
		int height = compact ? 14 : 18;
		int step = compact ? 0 : 22;
		return new int[] { leftX + (index % columns) * (cardWidth + gap),
				colorwayTop + 12 + (index / columns) * step, cardWidth, height };
	}

	private int[] surfaceRect(int index) {
		int gap = AetherDesignSystem.Metrics.SPACE_XS, buttonWidth = (leftW - gap * 2) / 3;
		return new int[] { leftX + index * (buttonWidth + gap), surfaceTop + 12, buttonWidth,
				compact ? 14 : 18 };
	}

	private int[] accentResetRect() {
		return new int[] { leftX + leftW - 44, accentTop - 15, 44, 13 };
	}

	private int[] swatchRect(int index) {
		int x = leftX + (index % swatchColumns) * (swatchSize + swatchGap);
		int y = accentTop + (index / swatchColumns) * (swatchSize + swatchGap);
		return new int[] { x, y, swatchSize, swatchSize };
	}

	private void renderColorways(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		AetherDesignSystem.Colorway selected = AetherDesignSystem.Colorway.fromId(Aether.get().getConfig().colorway);
		RenderUtil.text(graphics, font, "MATERIAL", leftX, colorwayTop, tone(AetherDesignSystem.TextRole.HINT));
		AetherDesignSystem.Colorway[] colorways = AetherDesignSystem.Colorway.values();
		for (int index = 0; index < colorways.length; index++) {
			AetherDesignSystem.Colorway colorway = colorways[index];
			int[] rect = colorwayRect(index);
			boolean chosen = colorway == selected, hovered = inside(mouseX, mouseY, rect);
			int fill = AetherDesignSystem.surface(palette, chosen ? AetherDesignSystem.SurfaceState.ACTIVE
					: hovered ? AetherDesignSystem.SurfaceState.HOVER : AetherDesignSystem.SurfaceState.REST);
			RenderUtil.bordered(graphics, rect[0], rect[1], rect[2], rect[3], SMALL_RADIUS, fade(fill),
					chosen ? fade(accent) : tone(AetherDesignSystem.TextRole.HINT));
			if (chosen) {
				RenderUtil.roundedRect(graphics, rect[0] + 1, rect[1] + 3, 2, 12, 1, fade(accent));
			}

			if (!compact) {
				int stripX = rect[0] + rect[2] - 17;
				graphics.fill(stripX, rect[1] + 7, stripX + 4, rect[1] + 11, colorway.palette().background());
				graphics.fill(stripX + 5, rect[1] + 7, stripX + 9, rect[1] + 11, colorway.palette().surfaceActive());
				graphics.fill(stripX + 10, rect[1] + 7, stripX + 14, rect[1] + 11, colorway.palette().accent());
			}

			String label = compact ? colorway.compactLabel() : colorway.label();
			RenderUtil.text(graphics, font,
					RenderUtil.truncate(font, label, rect[2] - (compact ? 10 : 38)),
					rect[0] + 8, rect[1] + (compact ? 3 : 5),
					chosen ? tone(AetherDesignSystem.TextRole.TITLE)
							: tone(AetherDesignSystem.TextRole.LABEL));
		}
	}

	private void renderSurfaceTreatments(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		AetherDesignSystem.SurfaceTreatment selected = AetherDesignSystem.surfaceTreatment(Aether.get().getConfig());
		RenderUtil.text(graphics, font, "SURFACE", leftX, surfaceTop, tone(AetherDesignSystem.TextRole.HINT));
		AetherDesignSystem.SurfaceTreatment[] treatments = AetherDesignSystem.SurfaceTreatment.values();
		for (int index = 0; index < treatments.length; index++) {
			AetherDesignSystem.SurfaceTreatment treatment = treatments[index];
			int[] rect = surfaceRect(index);
			boolean chosen = treatment == selected, hovered = inside(mouseX, mouseY, rect);
			int fill = chosen ? ColorUtil.lerp(palette.surfaceActive(), accent, 0.18f)
					: hovered ? palette.surfaceHover() : palette.surface();
			RenderUtil.bordered(graphics, rect[0], rect[1], rect[2], rect[3], SMALL_RADIUS, fade(fill),
					chosen ? fade(accent) : tone(AetherDesignSystem.TextRole.HINT));
			RenderUtil.centeredText(graphics, font, treatment.label(), rect[0] + rect[2] / 2,
					rect[1] + (rect[3] - 9) / 2, chosen ? tone(AetherDesignSystem.TextRole.TITLE)
							: tone(AetherDesignSystem.TextRole.LABEL));
		}
	}

	private void renderAccent(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		RenderUtil.text(graphics, font, "ACCENT", leftX, accentTop - 14, tone(AetherDesignSystem.TextRole.HINT));
		int[] reset = accentResetRect();
		boolean custom = Aether.get().getConfig().customAccent;
		boolean resetHovered = inside(mouseX, mouseY, reset);
		if (custom || resetHovered) {
			RenderUtil.roundedRect(graphics, reset[0], reset[1], reset[2], reset[3], SMALL_RADIUS,
					ColorUtil.withAlpha(accent, (resetHovered ? 0.14f : 0.07f) * opacity));
		}
		RenderUtil.centeredText(graphics, font, "Reset", reset[0] + reset[2] / 2, reset[1] + 2,
				custom ? tone(AetherDesignSystem.TextRole.ACCENT) : tone(AetherDesignSystem.TextRole.HINT));
		int[] presets = AetherDesignSystem.ACCENT_PRESETS;
		for (int i = 0; i < presets.length; i++) {
			int[] rect = swatchRect(i);
			RenderUtil.roundedRect(graphics, rect[0], rect[1], rect[2], rect[3], SMALL_RADIUS, presets[i]);
			graphics.fill(rect[0] + 3, rect[1] + 1, rect[0] + rect[2] - 3, rect[1] + 2,
					ColorUtil.withAlpha(palette.textPrimary(), 0.18f * opacity));
			boolean selected = custom && presets[i] == accent;
			boolean hovered = inside(mouseX, mouseY, rect);
			if (selected) {
				RenderUtil.outline(graphics, rect[0] - 2, rect[1] - 2, rect[2] + 4, rect[3] + 4,
						tone(AetherDesignSystem.TextRole.TITLE));
			} else if (hovered) {
				RenderUtil.outline(graphics, rect[0] - 1, rect[1] - 1, rect[2] + 2, rect[3] + 2,
						tone(AetherDesignSystem.TextRole.LABEL));
			}
		}
	}

	private void renderModules(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		RenderUtil.text(graphics, font, "KEYBINDS", rightX, bodyTop, tone(AetherDesignSystem.TextRole.HINT));
		String hint = rebinding == null ? "Click a module, then press a key" : "Press a key, Esc to cancel";
		// On a narrow window the hint would land on the label: it is the first thing to go.
		if (rightW - font.width("KEYBINDS") - 12 >= font.width(hint)) {
			RenderUtil.text(graphics, font, hint, rightX + rightW - font.width(hint), bodyTop,
					rebinding == null ? tone(AetherDesignSystem.TextRole.HINT) : fade(accent));
		}

		int contentH = listBottom - listTop;
		int total = modules.size() * ROW_H;
		maxScroll = Math.max(0, total - contentH);
		scroll = Math.max(0, Math.min(scroll, maxScroll));

		RenderUtil.glassInset(graphics, rightX - 5, listTop - 6, rightW + 10, contentH + 10, CARD_RADIUS,
				accent, opacity);

		graphics.enableScissor(rightX, listTop, rightX + rightW, listBottom);
		int cursor = listTop - scroll;
		for (Module module : modules) {
			if (cursor + ROW_H > listTop && cursor < listBottom) {
				boolean hovered = inside(mouseX, mouseY, rightX, cursor, rightW, ROW_H);
				boolean armed = module == rebinding;
				int fill = armed ? ColorUtil.lerp(palette.surfaceActive(), accent, 0.24f)
						: hovered ? palette.surfaceHover()
						: ColorUtil.lerp(palette.surface(), AetherDesignSystem.glassDeep(palette), 0.40f);
				int border = armed ? accent : hovered ? palette.borderStrong()
						: ColorUtil.withAlpha(palette.border(), 0.55f);
				RenderUtil.bordered(graphics, rightX, cursor + 1, rightW, ROW_H - 2, CARD_RADIUS,
						fade(fill), fade(border));
				if (armed) {
					RenderUtil.roundedRect(graphics, rightX + 1, cursor + 5, 2, ROW_H - 10, 1, fade(accent));
				}

				RenderUtil.roundedRect(graphics, rightX + 7, cursor + 8, 3, 3, 1,
						module.isEnabled() ? fade(accent) : tone(AetherDesignSystem.TextRole.HINT));
				int nameColour = module.isEnabled() || armed ? tone(AetherDesignSystem.TextRole.TITLE)
						: hovered ? tone(AetherDesignSystem.TextRole.TITLE)
						: tone(AetherDesignSystem.TextRole.LABEL);
				// The keycap is as wide as its label: the name gets what remains, never the other
				// way around. A bound key can be wide (Right Shift); the unbound dash is not.
				String key = keycapLabel(module);
				int keyWidth = font.width(key) + 10;
				boolean showKey = rightW >= keyWidth + 55;
				String name = RenderUtil.truncate(font, module.getName(),
						Math.max(18, rightW - (showKey ? keyWidth + 39 : 20)));
				RenderUtil.text(graphics, font, name, rightX + 15, cursor + 7, nameColour);

				if (showKey) {
					renderKeycap(graphics, font, module, cursor, accent);
				}
			}
			cursor += ROW_H;
		}
		graphics.disableScissor();

		if (total > contentH) {
			RenderUtil.scrollbar(graphics, rightX + rightW + 2, listTop, contentH, scroll, total, accent);
		}
	}

	private String keycapLabel(Module module) {
		if (module == rebinding) {
			return "\u2026";
		}
		if (module.getKeyBinding() == null) {
			return "\u2014";
		}
		// « Not Bound » is wider than the module names at the compact width; the dash is the
		// same shorthand the click GUI uses for a missing bind.
		if (compact && module.getKeyBinding().isUnbound()) {
			return "\u2014";
		}
		return module.getKeyBinding().getTranslatedKeyMessage().getString();
	}

	private void renderKeycap(GuiGraphics graphics, Font font, Module module, int rowY, int accent) {
		String key = keycapLabel(module);
		int w = font.width(key) + 10;
		int x = rightX + rightW - w - 8;
		int y = rowY + (ROW_H - 15) / 2;
		boolean armed = module == rebinding;
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		RenderUtil.chip(graphics, x, y, w, 15, SMALL_RADIUS,
				AetherDesignSystem.chip(palette, accent, armed),
				AetherDesignSystem.chipBorder(palette, accent, armed));
		RenderUtil.text(graphics, font, key, x + 5, y + 3,
				armed ? fade(accent) : tone(AetherDesignSystem.TextRole.HINT));
	}

	private void renderFooter(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		hairline(graphics, panelY + panelH - FOOTER_H);

		for (FooterButton button : footer) {
			boolean hovered = inside(mouseX, mouseY, button.x(), button.y(), button.w(), button.h());
			if (button.primary() && hovered) {
				RenderUtil.roundedRect(graphics, button.x() - 2, button.y() - 2, button.w() + 4,
						button.h() + 4, CONTROL_RADIUS + 2,
						ColorUtil.withAlpha(accent, 0.14f * opacity));
			}
			int fill = AetherDesignSystem.button(palette, accent, button.primary(), hovered);
			RenderUtil.bordered(graphics, button.x(), button.y(), button.w(), button.h(), CONTROL_RADIUS,
					fade(fill), fade(AetherDesignSystem.buttonBorder(palette, accent, button.primary())));
			RenderUtil.centeredText(graphics, font, button.label(), button.x() + button.w() / 2,
					button.y() + 9, fade(AetherDesignSystem.buttonText(palette, button.primary())));
		}
	}


	// ═══════════════════════════════════════════════════════════════════════ input

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		double mouseX = event.x();
		double mouseY = event.y();

		AetherDesignSystem.Colorway[] colorways = AetherDesignSystem.Colorway.values();
		for (int i = 0; i < colorways.length; i++) {
			if (inside(mouseX, mouseY, colorwayRect(i))) {
				Aether.get().getConfig().colorway = colorways[i].id();
				Aether.get().save();
				return true;
			}
		}
		AetherDesignSystem.SurfaceTreatment[] treatments = AetherDesignSystem.SurfaceTreatment.values();
		for (int i = 0; i < treatments.length; i++) {
			if (inside(mouseX, mouseY, surfaceRect(i))) {
				Aether.get().getConfig().surfaceTreatment = treatments[i].id();
				Aether.get().save();
				return true;
			}
		}
		if (inside(mouseX, mouseY, accentResetRect())) {
			Aether.get().getConfig().customAccent = false;
			Aether.get().save();
			return true;
		}
		int[] presets = AetherDesignSystem.ACCENT_PRESETS;
		for (int i = 0; i < presets.length; i++) {
			if (inside(mouseX, mouseY, swatchRect(i))) {
				Aether.get().getConfig().accentColor = ColorUtil.toHex(presets[i]);
				Aether.get().getConfig().customAccent = true;
				Aether.get().save();
				return true;
			}
		}

		if (inside(mouseX, mouseY, rightX, listTop, rightW, listBottom - listTop)) {
			int index = (int) ((mouseY - listTop + scroll) / ROW_H);
			if (index >= 0 && index < modules.size()) {
				rebinding = modules.get(index);
				return true;
			}
		}

		for (FooterButton button : footer) {
			if (!inside(mouseX, mouseY, button.x(), button.y(), button.w(), button.h())) {
				continue;
			}
			if (button.primary()) {
				onClose();
				return true;
			}
			switch (button.label()) {
				case "HUD editor" -> {
					Minecraft.getInstance().setScreen(new HudEditorScreen(this));
					return true;
				}
				case "Reset modules" -> {
					Aether.get().getModuleManager().reset();
					Aether.get().save();
					return true;
				}
				default -> {
					Aether.get().getConfig().resetSettings(Aether.get().getModuleManager());
					Aether.get().save();
					return true;
				}
			}
		}

		return super.mouseClicked(event, doubleClick);
	}

	@Override
	public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
		if (inside(mouseX, mouseY, rightX, listTop, rightW, listBottom - listTop)) {
			scroll = Math.max(0, Math.min(maxScroll, scroll - (int) scrollY * ROW_H));
		}
		return true;
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (rebinding != null) {
			if (event.key() == 256 || invalidKey(event)) {
				rebinding = null;
				return true;
			}
			// Every module owns a registered mapping from startup, so rebinding only moves its key:
			// creating one here would throw, Fabric refuses a registration made once the options of
			// the game exist.
			if (rebinding.getKeyBinding() != null) {
				rebinding.getKeyBinding().setKey(InputConstants.getKey(event));
				// setKey changes the field only: the keyboard handler looks the mapping up in a table
				// built from the keys at registration, so without this rebuild a rebound module kept
				// listening to its old key (and fired nothing at all when its default was unbound).
				KeyMapping.resetMapping();
				// Key mappings live in the game options, not in Aether's configuration: saving only
				// Aether left the new key in memory until a clean shutdown of the whole game.
				Minecraft.getInstance().options.save();
				Aether.get().save();
			}
			rebinding = null;
			return true;
		}
		if (event.key() == 256) {
			onClose();
			return true;
		}
		return super.keyPressed(event);
	}

	@Override
	public void onClose() {
		Aether.get().save();
		Minecraft.getInstance().setScreen(null);
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	// ══════════════════════════════════════════════════════════════════════ helpers

	/** A synthetic or platform event with no GLFW key and no real scancode must not be saved. */
	private static boolean invalidKey(KeyEvent event) {
		return event.key() == -1 && (event.scancode() < 0 || event.scancode() > 255);
	}

	private static boolean inside(double mx, double my, int[] rect) {
		return inside(mx, my, rect[0], rect[1], rect[2], rect[3]);
	}

	private static boolean inside(double mx, double my, int x, int y, int w, int h) {
		return mx >= x && mx < x + w && my >= y && my < y + h;
	}

	private float entrance() {
		float progress = Math.min(1.0f,
				Math.max(0.0f, (System.nanoTime() - openedAt) / (float) ENTRANCE_NANOS));
		float remaining = 1.0f - progress;
		return 1.0f - remaining * remaining * remaining;
	}

	private int fade(int colour) {
		return ColorUtil.fade(colour, opacity);
	}

	private int tone(AetherDesignSystem.TextRole role) {
		return ColorUtil.fade(AetherDesignSystem.text(RenderUtil.theme(), role), opacity);
	}
}

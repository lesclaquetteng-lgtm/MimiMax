package dev.aether.client.gui;

import com.mojang.blaze3d.platform.InputConstants;
import dev.aether.client.Aether;
import dev.aether.client.design.AetherDesignSystem;
import dev.aether.client.module.Module;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.util.SessionUtil;
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

	/**
	 * The accent presets: the graphite scale, then colour pulled a quarter of the way back to the
	 * background so the picker offers a choice without turning into a rainbow under a monochrome
	 * interface.
	 */
	private static final int[] PRESETS = {
		0xFFE7E9EC, 0xFFB9C0C8, 0xFF8C949C, 0xFF5E666E, 0xFF7E9C84, 0xFFC0A870,
		0xFFC4897F, 0xFF7E93A6, 0xFF938CB0, 0xFFB08A72, 0xFF7FA89A, 0xFFBFA57C
	};

	private static final int PANEL_W = 520;
	private static final int PANEL_H = 360;
	private static final int PAD = 16;
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
		// swatches shrink rather than run under the footer.
		colorwayTop = bodyTop + 62;
		surfaceTop = colorwayTop + 58;
		accentTop = surfaceTop + 58;
		int room = bodyBottom - accentTop;
		swatchSize = Math.max(12, Math.min(18, (room - (3 - 1) * SWATCH_GAP) / 3));

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
		graphics.fill(0, 0, width, height, ColorUtil.withAlpha(palette.background(), 0.82f));
		RenderUtil.bordered(graphics, panelX, panelY, panelW, panelH, 8,
				palette.elevatedBackground(), palette.border());
		renderHeader(graphics, font);
		renderSession(graphics, font, accent);
		renderColorways(graphics, font, mouseX, mouseY, accent);
		renderSurfaceTreatments(graphics, font, mouseX, mouseY, accent);
		renderAccent(graphics, font, mouseX, mouseY, accent);
		renderModules(graphics, font, mouseX, mouseY, accent);
		renderFooter(graphics, font, mouseX, mouseY, accent);

		// The vertical hairline that makes the two columns two columns.
		graphics.fill(leftX + leftW + COL_GAP / 2, bodyTop, leftX + leftW + COL_GAP / 2 + 1, bodyBottom,
			RenderUtil.theme().border());
	}

	private void renderHeader(GuiGraphics graphics, Font font) {
		RenderUtil.text(graphics, font, "Configuration", leftX, panelY + 14, RenderUtil.theme().textPrimary());
		String version = "v1.0";
		RenderUtil.text(graphics, font, version, panelX + panelW - PAD - font.width(version), panelY + 14,
			RenderUtil.theme().textDim());
		hairline(graphics, panelY + HEADER_H);
	}

	private void hairline(GuiGraphics graphics, int y) {
		graphics.fill(leftX, y, panelX + panelW - PAD, y + 1, RenderUtil.theme().border());
	}

	/** Avatar, name and the one bit that matters about the session: offline or Microsoft. */
	private void renderSession(GuiGraphics graphics, Font font, int accent) {
		RenderUtil.text(graphics, font, "Session", leftX, bodyTop, RenderUtil.theme().textDim());

		int y = bodyTop + 14;
		String username = SessionUtil.getUsername();
		String initials = username.isEmpty() ? "?" : username.substring(0, 1).toUpperCase();

		RenderUtil.roundedRect(graphics, leftX, y, 26, 26, 4, ColorUtil.withAlpha(accent, 0.18f));
		RenderUtil.text(graphics, font, initials, leftX + (26 - font.width(initials)) / 2, y + 8, accent);

		RenderUtil.text(graphics, font, username, leftX + 34, y + 2, RenderUtil.theme().textPrimary());

		boolean offline = SessionUtil.isOffline();
		int dot = offline ? 0xFFD9A441 : 0xFF46C98B;
		String state = offline ? "Offline" : "Microsoft";
		graphics.fill(leftX + 34, y + 17, leftX + 38, y + 21, dot);
		RenderUtil.text(graphics, font, state, leftX + 43, y + 14, RenderUtil.theme().textSecondary());

		hairline(graphics, y + 40);
	}

	private int[] colorwayRect(int index) {
		int gap = 6, cardWidth = (leftW - gap) / 2;
		return new int[] { leftX + (index % 2) * (cardWidth + gap),
				colorwayTop + 13 + (index / 2) * 22, cardWidth, 18 };
	}

	private int[] surfaceRect(int index) {
		int gap = 4, buttonWidth = (leftW - gap * 2) / 3;
		return new int[] { leftX + index * (buttonWidth + gap), surfaceTop + 13, buttonWidth, 18 };
	}

	private int[] accentResetRect() {
		return new int[] { leftX + leftW - 44, accentTop - 15, 44, 13 };
	}

	private int[] swatchRect(int index) {
		int x = leftX + (index % SWATCH_COLUMNS) * (swatchSize + SWATCH_GAP);
		int y = accentTop + (index / SWATCH_COLUMNS) * (swatchSize + SWATCH_GAP);
		return new int[] { x, y, swatchSize, swatchSize };
	}

	private void renderColorways(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		AetherDesignSystem.Colorway selected = AetherDesignSystem.Colorway.fromId(Aether.get().getConfig().colorway);
		RenderUtil.text(graphics, font, "Material", leftX, colorwayTop, palette.textDim());
		AetherDesignSystem.Colorway[] colorways = AetherDesignSystem.Colorway.values();
		for (int index = 0; index < colorways.length; index++) {
			AetherDesignSystem.Colorway colorway = colorways[index];
			int[] rect = colorwayRect(index);
			boolean chosen = colorway == selected, hovered = inside(mouseX, mouseY, rect);
			int fill = chosen ? palette.surfaceActive() : hovered ? palette.surfaceHover() : palette.surface();
			RenderUtil.bordered(graphics, rect[0], rect[1], rect[2], rect[3], 4, fill,
					chosen ? accent : palette.border());
			RenderUtil.roundedRect(graphics, rect[0] + 5, rect[1] + 5, 8, 8, 3, colorway.palette().accent());
			RenderUtil.text(graphics, font, colorway.label(), rect[0] + 17, rect[1] + 5,
					chosen ? palette.textPrimary() : palette.textSecondary());
		}
	}

	private void renderSurfaceTreatments(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		AetherDesignSystem.SurfaceTreatment selected = AetherDesignSystem.surfaceTreatment(Aether.get().getConfig());
		RenderUtil.text(graphics, font, "Surface", leftX, surfaceTop, palette.textDim());
		AetherDesignSystem.SurfaceTreatment[] treatments = AetherDesignSystem.SurfaceTreatment.values();
		for (int index = 0; index < treatments.length; index++) {
			AetherDesignSystem.SurfaceTreatment treatment = treatments[index];
			int[] rect = surfaceRect(index);
			boolean chosen = treatment == selected, hovered = inside(mouseX, mouseY, rect);
			int fill = chosen ? palette.surfaceActive() : hovered ? palette.surfaceHover() : palette.surface();
			RenderUtil.bordered(graphics, rect[0], rect[1], rect[2], rect[3], 4, fill,
					chosen ? accent : palette.border());
			RenderUtil.centeredText(graphics, font, treatment.label(), rect[0] + rect[2] / 2,
					rect[1] + 5, chosen ? palette.textPrimary() : palette.textSecondary());
		}
	}

	private void renderAccent(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		RenderUtil.text(graphics, font, "Accent", leftX, accentTop - 14, palette.textDim());
		int[] reset = accentResetRect();
		boolean custom = Aether.get().getConfig().customAccent;
		RenderUtil.text(graphics, font, "Reset", reset[0] + 12, reset[1] + 2,
				custom ? palette.accent() : palette.textDim());
		for (int i = 0; i < PRESETS.length; i++) {
			int[] rect = swatchRect(i);
			RenderUtil.roundedRect(graphics, rect[0], rect[1], rect[2], rect[3], 3, PRESETS[i]);
			boolean selected = custom && PRESETS[i] == accent;
			boolean hovered = inside(mouseX, mouseY, rect);
			if (selected) {
				RenderUtil.outline(graphics, rect[0] - 2, rect[1] - 2, rect[2] + 4, rect[3] + 4, palette.textPrimary());
			} else if (hovered) {
				RenderUtil.outline(graphics, rect[0] - 1, rect[1] - 1, rect[2] + 2, rect[3] + 2, palette.borderStrong());
			}
		}
	}

	private void renderModules(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		RenderUtil.text(graphics, font, "Keys", rightX, bodyTop, RenderUtil.theme().textDim());
		String hint = rebinding == null ? "Click a module, then press a key" : "Press a key, Esc to cancel";
		// On a narrow window the hint would land on the label: it is the first thing to go.
		if (rightW - font.width("Keys") - 12 >= font.width(hint)) {
			RenderUtil.text(graphics, font, hint, rightX + rightW - font.width(hint), bodyTop,
				rebinding == null ? RenderUtil.theme().textDim() : accent);
		}

		int contentH = listBottom - listTop;
		int total = modules.size() * ROW_H;
		maxScroll = Math.max(0, total - contentH);
		scroll = Math.max(0, Math.min(scroll, maxScroll));

		graphics.enableScissor(rightX, listTop, rightX + rightW, listBottom);
		int cursor = listTop - scroll;
		for (Module module : modules) {
			if (cursor + ROW_H > listTop && cursor < listBottom) {
				boolean hovered = inside(mouseX, mouseY, rightX, cursor, rightW, ROW_H);
				if (module == rebinding) {
					graphics.fill(rightX, cursor, rightX + rightW, cursor + ROW_H,
						ColorUtil.withAlpha(accent, 0.10f));
					graphics.fill(rightX, cursor, rightX + 2, cursor + ROW_H, accent);
				} else if (hovered) {
					graphics.fill(rightX, cursor, rightX + rightW, cursor + ROW_H, RenderUtil.theme().surfaceHover());
				}

				int nameColour = module.isEnabled() ? RenderUtil.theme().textPrimary() : RenderUtil.theme().textSecondary();
				String name = RenderUtil.truncate(font, module.getName(), rightW - 70);
				RenderUtil.text(graphics, font, name, rightX + 8, cursor + 7, nameColour);

				renderKeycap(graphics, font, module, cursor, accent);
			}
			cursor += ROW_H;
		}
		graphics.disableScissor();

		if (total > contentH) {
			RenderUtil.scrollbar(graphics, rightX + rightW - 2, listTop, contentH, scroll, total, accent);
		}
	}

	private void renderKeycap(GuiGraphics graphics, Font font, Module module, int rowY, int accent) {
		String key;
		if (module == rebinding) {
			key = "\u2026";
		} else if (module.getKeyBinding() == null) {
			key = "\u2014";
		} else {
			key = module.getKeyBinding().getTranslatedKeyMessage().getString();
		}

		int w = font.width(key) + 10;
		int x = rightX + rightW - w - 8;
		int y = rowY + (ROW_H - 15) / 2;
		boolean armed = module == rebinding;
		RenderUtil.bordered(graphics, x, y, w, 15, 3, RenderUtil.theme().surfaceHover(),
			armed ? accent : RenderUtil.theme().borderStrong());
		RenderUtil.text(graphics, font, key, x + 5, y + 3, armed ? accent : RenderUtil.theme().textDim());
	}

	private void renderFooter(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		hairline(graphics, panelY + panelH - FOOTER_H);

		for (FooterButton button : footer) {
			boolean hovered = inside(mouseX, mouseY, button.x(), button.y(), button.w(), button.h());
			// A fill is always opaque here: `bordered` paints the border as a full rectangle, and a
			// transparent fill would let it show through as a solid slab.
			int fill = button.primary()
				? (hovered ? ColorUtil.lerp(accent, 0xFFFFFFFF, 0.12f) : accent)
				: (hovered ? RenderUtil.theme().surfaceHover() : RenderUtil.theme().surface());
			RenderUtil.bordered(graphics, button.x(), button.y(), button.w(), button.h(), 4, fill,
				button.primary() ? accent : RenderUtil.theme().borderStrong());
			RenderUtil.centeredText(graphics, font, button.label(), button.x() + button.w() / 2,
				button.y() + 9, button.primary() ? 0xFF0A0B0D : RenderUtil.theme().textSecondary());
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
		for (int i = 0; i < PRESETS.length; i++) {
			if (inside(mouseX, mouseY, swatchRect(i))) {
				Aether.get().getConfig().accentColor = ColorUtil.toHex(PRESETS[i]);
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
			if (event.key() == 256) {
				rebinding = null;
				return true;
			}
			// Every module owns a registered mapping from startup, so rebinding only moves its key:
			// creating one here would throw, Fabric refuses a registration made once the options of
			// the game exist.
			if (rebinding.getKeyBinding() != null) {
				rebinding.getKeyBinding().setKey(InputConstants.getKey(event));
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

	private static boolean inside(double mx, double my, int[] rect) {
		return inside(mx, my, rect[0], rect[1], rect[2], rect[3]);
	}

	private static boolean inside(double mx, double my, int x, int y, int w, int h) {
		return mx >= x && mx < x + w && my >= y && my < y + h;
	}
}

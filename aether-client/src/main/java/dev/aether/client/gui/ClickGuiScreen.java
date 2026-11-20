package dev.aether.client.gui;

import com.mojang.blaze3d.platform.InputConstants;
import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.ColorSetting;
import dev.aether.client.setting.DoubleSetting;
import dev.aether.client.setting.IntegerSetting;
import dev.aether.client.setting.ModeSetting;
import dev.aether.client.setting.Setting;
import dev.aether.client.util.Animation;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

/**
 * The Click GUI: a fixed rail of categories on the left, the modules of the selected category in a
 * readable list on the right.
 *
 * <p>The screen is a single layout function away from its input handling: {@link #rows()} computes
 * every rectangle once per frame, drawing and clicking both read it. The previous design computed
 * the position of a card twice — once to paint it, once to hit-test it — and the two drifted (a
 * slider painted ten pixels away from the place a click was read). Nothing here measures the same
 * thing twice.</p>
 */
public class ClickGuiScreen extends Screen {

	// ── Panel geometry ────────────────────────────────────────────────────────
	private static final int PANEL_MAX_WIDTH = 820;
	private static final int PANEL_MAX_HEIGHT = 560;
	private static final int RAIL_WIDTH = 196;
	private static final int MARGIN = 16;
	private static final int HEADER_HEIGHT = 56;

	// ── Rows ──────────────────────────────────────────────────────────────────
	private static final int ROW_HEIGHT = 46;
	private static final int ROW_GAP = 6;
	private static final int SETTING_HEIGHT = 22;
	private static final int SWITCH_WIDTH = 30;
	private static final int SWITCH_HEIGHT = 16;
	private static final int CHIP_HEIGHT = 18;

	// ── Sliders: one geometry for the paint and the click ─────────────────────
	private static final int SLIDER_WIDTH = 84;
	private static final int SLIDER_LABEL_WIDTH = 44;

	// ── Search field ──────────────────────────────────────────────────────────
	private static final int SEARCH_WIDTH = 180;
	private static final int SEARCH_HEIGHT = 20;

	// ── Category rail ─────────────────────────────────────────────────────────
	private static final int CATEGORY_HEIGHT = 30;
	private static final int CATEGORY_STEP = 34;

	// ── Animations ────────────────────────────────────────────────────────────
	private static final long ENTRANCE_NANOS = 200_000_000L;
	private static final int[] PALETTE = {
			0x6C5CE7, 0x2ECC8F, 0xF0B429, 0xE05260, 0x3498DB, 0xE67E22, 0xFF6CE8, 0x00D2D3
	};

	private final Screen previousScreen;
	private ModuleCategory selectedCategory;
	private final List<Module> visible = new ArrayList<>();
	private final Set<Module> expanded = new LinkedHashSet<>();

	private String search = "";
	private boolean searchFocused;

	private float scroll;
	private float scrollTarget;
	private float maxScroll;
	private float contentHeight;
	private long lastFrameNanos;
	private final long openedAt = System.nanoTime();
	private float opacity = 1.0f;

	private final Animation pill = new Animation(0.0f, 20.0f);
	private boolean pillPlaced;
	private long lastPillNanos;

	private Setting dragged;
	private int dragX;
	private int dragWidth;
	private Module rebinding;

	// ── Computed once by init(), used by every frame and every click ──────────
	private int panelX;
	private int panelY;
	private int panelWidth;
	private int panelHeight;
	private int railX;
	private int railY;
	private int railWidth;

	// ── Resolved by init(): a small window gets a tighter screen, never a clipped one ──
	private boolean compact;
	private int rowHeight;
	private int categoryHeight;
	private int categoryStep;
	private int railTop;
	private int searchWidth;
	private int listX;
	private int listY;
	private int listWidth;
	private int listHeight;

	private record Row(Module module, int x, int y, int width, int height, boolean expanded) {
	}

	public ClickGuiScreen(ModuleCategory category) {
		this(category, null);
	}

	public ClickGuiScreen(ModuleCategory category, Screen previousScreen) {
		super(Component.literal("Aether"));
		this.selectedCategory = category == null ? ModuleCategory.COMBAT : category;
		this.previousScreen = previousScreen;
	}

	@Override
	protected void init() {
		panelWidth = Math.min(width - MARGIN * 2, PANEL_MAX_WIDTH);
		panelHeight = Math.min(height - MARGIN * 2, PANEL_MAX_HEIGHT);
		panelX = (width - panelWidth) / 2;
		panelY = (height - panelHeight) / 2;

		railX = panelX + 12;
		railY = panelY + 12;
		// The rail takes its full width while the window allows it and shrinks before the list
		// does: a narrow window loses padding, never the modules.
		railWidth = Math.max(120, Math.min(RAIL_WIDTH, panelWidth - 260));
		listX = panelX + railWidth + 12 + MARGIN;
		listWidth = panelWidth - (railWidth + 12 + MARGIN) - MARGIN;
		listY = panelY + HEADER_HEIGHT;
		listHeight = panelHeight - HEADER_HEIGHT - MARGIN;

		// A window at a high interface scale leaves few pixels: the rows lose their second line
		// and the rail its version, instead of a row or a category being clipped away.
		compact = panelHeight < 360 || panelWidth < 500;
		rowHeight = compact ? 36 : ROW_HEIGHT;
		categoryHeight = compact ? 24 : CATEGORY_HEIGHT;
		categoryStep = compact ? 28 : CATEGORY_STEP;
		railTop = compact ? 38 : 66;

		// The search shrinks instead of covering the title of the category.
		searchWidth = Math.max(96, Math.min(SEARCH_WIDTH, listWidth - 150));

		scroll = 0.0f;
		scrollTarget = 0.0f;
		rebuild();
	}

	// ══════════════════════════════════════════════════════════════════════════
	//  Layout — the single source of every rectangle
	// ══════════════════════════════════════════════════════════════════════════

	private void rebuild() {
		visible.clear();
		String query = search.trim().toLowerCase(Locale.ROOT);
		for (Module module : Aether.get().getModuleManager().getByCategory(selectedCategory)) {
			if (query.isEmpty()
					|| module.getName().toLowerCase(Locale.ROOT).contains(query)
					|| module.getDescription().toLowerCase(Locale.ROOT).contains(query)) {
				visible.add(module);
			}
		}
		maxScroll = 0.0f;
		scroll = 0.0f;
		scrollTarget = 0.0f;
	}

	private List<Row> rows() {
		List<Row> rows = new ArrayList<>();
		int offset = Math.round(scroll);
		int y = listY - offset;

		for (Module module : visible) {
			int settings = expanded.contains(module) && !module.getSettings().isEmpty()
					? module.getSettings().size()
					: 0;
			int height = rowHeight + (settings > 0 ? 8 + settings * SETTING_HEIGHT + 8 : 0);
			rows.add(new Row(module, listX, y, listWidth, height, settings > 0));
			y += height + ROW_GAP;
		}

		contentHeight = Math.max(1.0f, y + offset - listY);
		maxScroll = Math.max(0.0f, contentHeight - listHeight);
		return rows;
	}

	private static int settingX(Row row) {
		return row.x + 14;
	}

	private int settingY(Row row, int index) {
		return row.y + rowHeight + 8 + index * SETTING_HEIGHT;
	}

	private static int settingWidth(Row row) {
		// The text starts 14 in and the controls end MARGIN before the right edge, so the two
		// sides of a settings row are symmetric and line up with the switch of the module row.
		return row.width - 14 - MARGIN;
	}

	/** The x of the slider of a numeric setting, shared by the paint and the click. */
	private static int sliderX(int settingX, int settingWidth) {
		return settingX + settingWidth - SLIDER_WIDTH - SLIDER_LABEL_WIDTH;
	}

	private static int toggleX(Row row) {
		return row.x + row.width - MARGIN - SWITCH_WIDTH;
	}

	private int toggleY(Row row) {
		return row.y + (rowHeight - SWITCH_HEIGHT) / 2;
	}

	private static int chevronX(Row row) {
		return row.x + row.width - MARGIN - SWITCH_WIDTH - 20;
	}

	private int chipTop(Row row) {
		return row.y + (rowHeight - CHIP_HEIGHT) / 2;
	}

	// ══════════════════════════════════════════════════════════════════════════
	//  Render
	// ══════════════════════════════════════════════════════════════════════════

	private float entrance() {
		float progress = Math.min(1.0f, (System.nanoTime() - openedAt) / (float) ENTRANCE_NANOS);
		float remaining = 1.0f - progress;
		return 1.0f - remaining * remaining * remaining;
	}

	private void advanceAnimations() {
		long now = System.nanoTime();
		long delta = lastFrameNanos == 0 ? 16 : Math.min(100, (now - lastFrameNanos) / 1_000_000);
		lastFrameNanos = now;

		float factor = 1.0f - (float) Math.pow(0.001, (delta / 1000.0) * 16.0f);
		scroll += (scrollTarget - scroll) * factor;
		if (Math.abs(scrollTarget - scroll) < 0.5f) {
			scroll = scrollTarget;
		}
		scroll = Math.max(0.0f, Math.min(maxScroll, scroll));

		// The switch of each row is advanced on the frame clock, never from the tick.
		Aether.get().getModuleManager().updateAnimations(delta);

		long pillDelta = lastPillNanos == 0 ? 16 : Math.min(100, (now - lastPillNanos) / 1_000_000);
		lastPillNanos = now;
		pill.update(pillDelta);
	}

	@Override
	public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {
		Font font = Minecraft.getInstance().font;
		int accent = Aether.get().getConfig().getAccentColor();

		advanceAnimations();
		opacity = entrance();

		List<Row> rows = rows();
		renderBackdrop(graphics);
		renderPanel(graphics, accent);
		renderRail(graphics, font, mouseX, mouseY, accent);
		renderHeader(graphics, font, mouseX, mouseY, accent);
		renderRows(graphics, font, rows, mouseX, mouseY, accent);
		renderScrollbar(graphics, accent);
	}

	private void renderBackdrop(GuiGraphics graphics) {
		graphics.fill(0, 0, width, height, fade(0xCC07080A));
	}

	private void renderPanel(GuiGraphics graphics, int accent) {
		RenderUtil.shadow(graphics, panelX, panelY, panelWidth, panelHeight, 12);
		RenderUtil.bordered(graphics, panelX, panelY, panelWidth, panelHeight, 8,
				fade(0xFA101216), fade(0xFF21252B));

		// The rail sits on its own surface, one shade darker than the list. A hairline separates
		// them: no gradient, no glow, the same language as the launcher.
		RenderUtil.roundedRect(graphics, railX, railY, railWidth, panelHeight - 24, 8, fade(0xFF0A0C0F));
		graphics.fill(railX + railWidth - 1, railY + 12, railX + railWidth, railY + panelHeight - 36,
				fade(0x1AFFFFFF));
	}

	private void renderRail(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		int x = railX + 16;
		int y = railY + (compact ? 10 : 18);

		RenderUtil.roundedRect(graphics, x, y + 3, 6, 6, 3, ColorUtil.withAlpha(accent, opacity));
		RenderUtil.text(graphics, font, "AETHER", x + 12, y, fade(RenderUtil.TEXT_PRIMARY));
		if (!compact) {
			RenderUtil.text(graphics, font, "v" + Aether.VERSION, x, y + 14, fade(RenderUtil.TEXT_DIM));
		}

		ModuleCategory[] categories = ModuleCategory.values();
		int itemX = railX + 10;
		int itemWidth = railWidth - 20;

		// Sliding pill first, so the labels keep their own colour, then the labels.
		// The first paint places it, later frames animate it on the frame clock.
		int selectedIndex = 0;
		for (int index = 0; index < categories.length; index++) {
			if (categories[index] == selectedCategory) {
				selectedIndex = index;
			}
		}
		int targetY = categoryTop(selectedIndex);
		if (!pillPlaced) {
			pill.set(targetY);
			pillPlaced = true;
		} else if (Math.abs(pill.getTarget() - targetY) > 0.5f) {
			pill.setTarget(targetY);
		}
		int pillY = Math.round(pill.getValue());
		RenderUtil.roundedRect(graphics, itemX, pillY, itemWidth, categoryHeight, 7,
				ColorUtil.withAlpha(accent, 0.16f * opacity));
		RenderUtil.roundedRect(graphics, itemX, pillY + (categoryHeight - 12) / 2, 2, 12, 1,
				ColorUtil.withAlpha(accent, opacity));

		for (int index = 0; index < categories.length; index++) {
			ModuleCategory category = categories[index];
			int itemY = categoryTop(index);
			boolean active = category == selectedCategory;
			boolean hovered = inside(mouseX, mouseY, itemX, itemY, itemWidth, categoryHeight);

			if (!active && hovered) {
				RenderUtil.roundedRect(graphics, itemX, itemY, itemWidth, categoryHeight, 7,
						fade(0x10FFFFFF));
			}

			int count = Aether.get().getModuleManager().getByCategory(category).size();
			String label = category.getDisplayName();
			int textColour = active ? ColorUtil.withAlpha(accent, opacity)
					: hovered ? fade(RenderUtil.TEXT_PRIMARY) : fade(RenderUtil.TEXT_MUTED);
			int labelY = itemY + (categoryHeight - 9) / 2;
			RenderUtil.text(graphics, font, label, itemX + 12, labelY, textColour);
			String badge = String.valueOf(count);
			RenderUtil.text(graphics, font, badge, itemX + itemWidth - 12 - font.width(badge), labelY,
					fade(RenderUtil.TEXT_DIM));
		}

		if (panelHeight > 320) {
			RenderUtil.text(graphics, font, "ESC  close", railX + 16, railY + panelHeight - 40,
					fade(RenderUtil.TEXT_DIM));
		}
	}

	private void renderHeader(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		int headerY = panelY + 20;
		RenderUtil.text(graphics, font, selectedCategory.getDisplayName().toUpperCase(Locale.ROOT),
				listX, headerY, fade(RenderUtil.TEXT_PRIMARY));

		int titleWidth = font.width(selectedCategory.getDisplayName().toUpperCase(Locale.ROOT));
		String count = visible.size() + (visible.size() == 1 ? " module" : " modules");
		if (searchX() - (listX + titleWidth + 10) > font.width(count) + 12) {
			RenderUtil.text(graphics, font, count, listX + titleWidth + 10, headerY, fade(RenderUtil.TEXT_DIM));
		}

		int searchX = searchX();
		boolean hovered = inside(mouseX, mouseY, searchX, searchTop(), searchWidth, SEARCH_HEIGHT);
		RenderUtil.bordered(graphics, searchX, searchTop(), searchWidth, SEARCH_HEIGHT, 6,
				fade(0xFF0A0A0E), searchFocused ? accent : fade(hovered ? 0x44FFFFFF : 0x22FFFFFF));

		String text = search.isEmpty() ? "Search" : search;
		int colour = search.isEmpty() ? fade(RenderUtil.TEXT_DIM) : fade(RenderUtil.TEXT_PRIMARY);
		RenderUtil.text(graphics, font, RenderUtil.truncate(font, text, searchWidth - 16), searchX + 8,
				searchTop() + 7, colour);
	}

	private void renderRows(GuiGraphics graphics, Font font, List<Row> rows, int mouseX, int mouseY, int accent) {
		graphics.enableScissor(listX - 4, listY - 4, listX + listWidth + 4, listY + listHeight + 4);

		for (Row row : rows) {
			if (row.y + row.height < listY - ROW_GAP || row.y > listY + listHeight + ROW_GAP) {
				continue;
			}
			renderRow(graphics, font, row, mouseX, mouseY, accent);
		}

		graphics.disableScissor();

		if (visible.isEmpty()) {
			String message = search.isEmpty() ? "No module in this category" : "Nothing matches \"" + search + "\"";
			RenderUtil.centeredText(graphics, font, message, listX + listWidth / 2, listY + 40,
					fade(RenderUtil.TEXT_DIM));
		}
	}

	private void renderRow(GuiGraphics graphics, Font font, Row row, int mouseX, int mouseY, int accent) {
		Module module = row.module;
		boolean enabled = module.isEnabled();
		boolean hovered = inside(mouseX, mouseY, row.x, row.y, row.width, rowHeight);

		// The accent tints the surface instead of being mixed with its alpha: blending a
		// translucent accent into an opaque surface produced a saturated block of colour.
		int tint = ColorUtil.lerp(0xFF14161A, 0xFF000000 | (accent & 0xFFFFFF), 0.14f);
		int background = enabled ? fade(tint) : fade(hovered ? 0xFF191C21 : 0xFF14161A);
		int border = enabled ? ColorUtil.withAlpha(accent, 0.38f * opacity)
				: fade(hovered ? 0x33FFFFFF : 0x1AFFFFFF);

		RenderUtil.bordered(graphics, row.x, row.y, row.width, rowHeight, 8, background, border);
		if (enabled) {
			RenderUtil.roundedRect(graphics, row.x + 1, row.y + 12, 2, rowHeight - 24, 1,
					ColorUtil.withAlpha(accent, opacity));
		}

		int nameColour = enabled ? fade(0xFFFFFFFF)
				: hovered ? fade(RenderUtil.TEXT_PRIMARY) : fade(RenderUtil.TEXT_MUTED);
		int nameY = row.y + (compact ? (rowHeight - 9) / 2 : 9);
		RenderUtil.text(graphics, font, module.getName(), row.x + 14, nameY, nameColour);

		if (!compact && row.width > 300) {
			RenderUtil.text(graphics, font, RenderUtil.truncate(font, module.getDescription(), row.width - 200),
					row.x + 14, row.y + 26, fade(RenderUtil.TEXT_DIM));
		}

		// Keybind chip, left of the chevron.
		KeyMapping mapping = module.getKeyBinding();
		if (mapping != null && (rebinding == module || !mapping.isUnbound())) {
			boolean capturing = rebinding == module;
			String label = capturing ? "key?" : mapping.getTranslatedKeyMessage().getString();
			int chipWidth = font.width(label) + 10;
			int chipX = chevronX(row) - 8 - chipWidth;
			RenderUtil.roundedRect(graphics, chipX, chipTop(row), chipWidth, CHIP_HEIGHT, 4,
					capturing ? ColorUtil.withAlpha(accent, 0.5f * opacity) : fade(0x14FFFFFF));
			RenderUtil.text(graphics, font, label, chipX + 5, chipTop(row) + 5,
					capturing ? fade(0xFFFFFFFF) : fade(RenderUtil.TEXT_DIM));
		}

		// Chevron: opens the settings of the row.
		if (!module.getSettings().isEmpty()) {
			String chevron = row.expanded ? "\u25B4" : "\u25BE";
			int chevronColour = hovered ? fade(RenderUtil.TEXT_PRIMARY) : fade(RenderUtil.TEXT_DIM);
			RenderUtil.centeredText(graphics, font, chevron, chevronX(row) + 6, row.y + (rowHeight - 9) / 2,
					chevronColour);
		}

		drawSwitch(graphics, toggleX(row), toggleY(row), module.getToggleProgress(), accent);

		if (row.expanded) {
			graphics.fill(row.x + 14, row.y + rowHeight, row.x + row.width - 14, row.y + rowHeight + 1,
					fade(0x14FFFFFF));
			List<Setting> settings = module.getSettings();
			for (int index = 0; index < settings.size(); index++) {
				renderSetting(graphics, font, row, settings.get(index), index, mouseX, mouseY, accent);
			}
		}
	}

	private void drawSwitch(GuiGraphics graphics, int x, int y, float progress, int accent) {
		int track = ColorUtil.lerp(fade(0x33FFFFFF), ColorUtil.withAlpha(accent, 0.95f * opacity), progress);
		RenderUtil.roundedRect(graphics, x, y, SWITCH_WIDTH, SWITCH_HEIGHT, SWITCH_HEIGHT / 2, track);

		int knob = SWITCH_HEIGHT - 4;
		int knobX = x + 2 + Math.round((SWITCH_WIDTH - knob - 4) * progress);
		RenderUtil.roundedRect(graphics, knobX, y + 2, knob, knob, knob / 2, fade(0xFFFFFFFF));
	}

	private void renderSetting(GuiGraphics graphics, Font font, Row row, Setting setting, int index,
			int mouseX, int mouseY, int accent) {
		int x = settingX(row);
		int y = settingY(row, index);
		int w = settingWidth(row);
		boolean hovered = inside(mouseX, mouseY, x - 4, y, w + 8, SETTING_HEIGHT);

		if (hovered) {
			RenderUtil.roundedRect(graphics, x - 4, y, w + 8, SETTING_HEIGHT - 2, 4, fade(0x0CFFFFFF));
		}

		RenderUtil.text(graphics, font, setting.getName(), x + 4, y + 6,
				hovered ? fade(RenderUtil.TEXT_PRIMARY) : fade(RenderUtil.TEXT_MUTED));

		if (setting instanceof BooleanSetting toggle) {
			drawSwitch(graphics, x + w - SWITCH_WIDTH, y + 3, toggle.isEnabled() ? 1.0f : 0.0f, accent);
		} else if (setting instanceof IntegerSetting number) {
			float ratio = (number.get() - number.getMin())
					/ (float) Math.max(1, number.getMax() - number.getMin());
			RenderUtil.bar(graphics, sliderX(x, w), y + 9, SLIDER_WIDTH, 3, ratio,
					fade(0x26FFFFFF), ColorUtil.withAlpha(accent, opacity));
			String value = String.valueOf(number.get());
			RenderUtil.text(graphics, font, value, x + w - font.width(value), y + 6,
					fade(RenderUtil.TEXT_PRIMARY));
		} else if (setting instanceof DoubleSetting number) {
			float ratio = (float) ((number.get() - number.getMin())
					/ Math.max(0.0001, number.getMax() - number.getMin()));
			RenderUtil.bar(graphics, sliderX(x, w), y + 9, SLIDER_WIDTH, 3, ratio,
					fade(0x26FFFFFF), ColorUtil.withAlpha(accent, opacity));
			String value = String.format(Locale.ROOT, "%.2f", number.get());
			RenderUtil.text(graphics, font, value, x + w - font.width(value), y + 6,
					fade(RenderUtil.TEXT_PRIMARY));
		} else if (setting instanceof ModeSetting mode) {
			String value = mode.get();
			int chipWidth = font.width(value) + 16;
			RenderUtil.roundedRect(graphics, x + w - chipWidth, y + 4, chipWidth, 14, 4,
					ColorUtil.withAlpha(accent, 0.16f * opacity));
			RenderUtil.centeredText(graphics, font, value, x + w - chipWidth / 2, y + 7,
					ColorUtil.withAlpha(accent, opacity));
		} else if (setting instanceof ColorSetting colour) {
			RenderUtil.roundedRect(graphics, x + w - 14, y + 4, 14, 14, 3, colour.getColor());
		}
	}

	private void renderScrollbar(GuiGraphics graphics, int accent) {
		if (maxScroll <= 0.0f) {
			return;
		}
		int trackX = listX + listWidth + 4;
		int thumbHeight = Math.max(24, Math.round(listHeight * (listHeight / contentHeight)));
		int travel = listHeight - thumbHeight;
		int thumbY = listY + (maxScroll <= 0.0f ? 0 : Math.round(travel * (scroll / maxScroll)));

		RenderUtil.roundedRect(graphics, trackX, listY, 2, listHeight, 1, fade(0x14FFFFFF));
		RenderUtil.roundedRect(graphics, trackX, thumbY, 2, thumbHeight, 1,
				ColorUtil.withAlpha(accent, 0.75f * opacity));
	}

	// ══════════════════════════════════════════════════════════════════════════
	//  Input
	// ══════════════════════════════════════════════════════════════════════════

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		int mouseX = (int) event.x();
		int mouseY = (int) event.y();

		// Search field.
		if (inside(mouseX, mouseY, searchX(), searchTop(), searchWidth, SEARCH_HEIGHT)) {
			searchFocused = true;
			return true;
		}
		searchFocused = false;

		// Category rail.
		ModuleCategory[] categories = ModuleCategory.values();
		for (int index = 0; index < categories.length; index++) {
			if (inside(mouseX, mouseY, railX + 10, categoryTop(index), railWidth - 20,
					categoryHeight)) {
				if (categories[index] != selectedCategory) {
					selectedCategory = categories[index];
					rebuild();
				}
				return true;
			}
		}

		// Rows.
		// A row scrolled out of the list is not drawn: it must not be clickable either.
		if (!inside(mouseX, mouseY, listX, listY, listWidth, listHeight)) {
			return super.mouseClicked(event, doubleClick);
		}

		for (Row row : rows()) {
			Module module = row.module;

			// Settings of an expanded row first: they sit on top of nothing else.
			List<Setting> settings = module.getSettings();
			if (row.expanded) {
				for (int index = 0; index < settings.size(); index++) {
					Setting setting = settings.get(index);
					int x = settingX(row);
					int y = settingY(row, index);
					int w = settingWidth(row);
					if (!inside(mouseX, mouseY, x - 4, y, w + 8, SETTING_HEIGHT)) {
						continue;
					}
					boolean numeric = setting instanceof IntegerSetting || setting instanceof DoubleSetting;
					// A numeric setting is edited on its bar only: a click on the name used to be
					// read as a click on the far left of the bar and reset the value.
					if (numeric && !doubleClick
							&& !inside(mouseX, mouseY, sliderX(x, w) - 4, y, SLIDER_WIDTH + 8, SETTING_HEIGHT)) {
						return true;
					}
					editSetting(setting, doubleClick, mouseX, x, w, event.button());
					Aether.get().save();
					if (!doubleClick && numeric) {
						dragged = setting;
						dragX = sliderX(x, w);
						dragWidth = SLIDER_WIDTH;
					}
					return true;
				}
			}

			if (!inside(mouseX, mouseY, row.x, row.y, row.width, rowHeight)) {
				continue;
			}

			// Keybind chip.
			KeyMapping mapping = module.getKeyBinding();
			if (mapping != null && (rebinding == module || !mapping.isUnbound())) {
				String label = rebinding == module ? "key?" : mapping.getTranslatedKeyMessage().getString();
				int chipWidth = Minecraft.getInstance().font.width(label) + 10;
				int chipX = chevronX(row) - 8 - chipWidth;
				if (inside(mouseX, mouseY, chipX, chipTop(row), chipWidth, CHIP_HEIGHT)) {
					rebinding = rebinding == module ? null : module;
					return true;
				}
			}

			// Chevron.
			if (!settings.isEmpty() && inside(mouseX, mouseY, chevronX(row) - 4, row.y + 12, 22, 22)) {
				toggleExpanded(module);
				return true;
			}

			// Right click opens the settings, left click toggles the module.
			if (event.button() == 1) {
				if (!settings.isEmpty()) {
					toggleExpanded(module);
				}
				return true;
			}

			module.toggle();
			Aether.get().save();
			return true;
		}

		return super.mouseClicked(event, doubleClick);
	}

	private void toggleExpanded(Module module) {
		if (!expanded.remove(module)) {
			expanded.add(module);
		}
	}

	private void editSetting(Setting setting, boolean doubleClick, int mouseX, int x, int width, int button) {
		if (doubleClick) {
			setting.reset();
			return;
		}
		if (setting instanceof BooleanSetting toggle) {
			toggle.toggle();
		} else if (setting instanceof ModeSetting mode) {
			if (button == 1) {
				mode.cycleBack();
			} else {
				mode.cycle();
			}
		} else if (setting instanceof ColorSetting colour) {
			int index = 0;
			for (int i = 0; i < PALETTE.length; i++) {
				if (PALETTE[i] == colour.getColor()) {
					index = i;
					break;
				}
			}
			colour.setValue(PALETTE[(index + 1) % PALETTE.length]);
		} else {
			setSliderFromPos(setting, mouseX, sliderX(x, width), SLIDER_WIDTH);
		}
	}

	private void setSliderFromPos(Setting setting, double mouseX, int barX, int barWidth) {
		if (barWidth <= 0) {
			return;
		}
		float ratio = (float) Math.max(0.0, Math.min(1.0, (mouseX - barX) / barWidth));
		if (setting instanceof IntegerSetting number) {
			number.setValue(number.getMin() + Math.round((number.getMax() - number.getMin()) * ratio));
		} else if (setting instanceof DoubleSetting number) {
			double value = number.getMin() + (number.getMax() - number.getMin()) * ratio;
			number.setValue(Math.round(value * 100.0) / 100.0);
		}
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dragX, double dragY) {
		if (dragged != null) {
			setSliderFromPos(dragged, event.x(), this.dragX, this.dragWidth);
			return true;
		}
		return super.mouseDragged(event, dragX, dragY);
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		if (dragged != null) {
			dragged = null;
			Aether.get().save();
			return true;
		}
		return super.mouseReleased(event);
	}

	@Override
	public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
		if (!inside(mouseX, mouseY, listX, listY, listWidth, listHeight)) {
			return super.mouseScrolled(mouseX, mouseY, scrollX, scrollY);
		}
		scrollTarget -= (float) scrollY * 32.0f;
		scrollTarget = Math.max(0.0f, Math.min(maxScroll, scrollTarget));
		return true;
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (rebinding != null) {
			KeyMapping mapping = rebinding.getKeyBinding();
			if (mapping != null) {
				// ESC clears the bind, any other key replaces it. The mapping is moved, never
				// recreated: Fabric refuses a registration made once the options exist.
				mapping.setKey(event.key() == 256 ? InputConstants.UNKNOWN : InputConstants.getKey(event));
			}
			rebinding = null;
			Aether.get().save();
			return true;
		}

		if (event.key() == 256) {
			onClose();
			return true;
		}
		if (searchFocused) {
			if (event.key() == 259 && !search.isEmpty()) {
				search = search.substring(0, search.length() - 1);
				rebuild();
				return true;
			}
			if (event.key() == 257 || event.key() == 258) {
				searchFocused = false;
				return true;
			}
		}
		return super.keyPressed(event);
	}

	@Override
	public boolean charTyped(CharacterEvent event) {
		if (searchFocused && event.isAllowedChatCharacter() && search.length() < 32) {
			search += event.codepointAsString();
			rebuild();
			return true;
		}
		return super.charTyped(event);
	}

	@Override
	public void onClose() {
		Aether.get().save();
		Minecraft.getInstance().setScreen(previousScreen);
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	// ══════════════════════════════════════════════════════════════════════════
	//  Helpers
	// ══════════════════════════════════════════════════════════════════════════

	private int searchX() {
		return listX + listWidth - searchWidth;
	}

	private int searchTop() {
		return panelY + 14;
	}

	/** The top of the category at the given index, shared by the paint and the click. */
	private int categoryTop(int index) {
		return railY + railTop + index * categoryStep;
	}

	private static boolean inside(double mouseX, double mouseY, int x, int y, int width, int height) {
		return mouseX >= x && mouseX <= x + width && mouseY >= y && mouseY <= y + height;
	}

	private int fade(int colour) {
		return ColorUtil.fade(colour, opacity);
	}
}

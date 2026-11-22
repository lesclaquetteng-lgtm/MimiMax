package dev.aether.client.gui;

import dev.aether.client.Aether;
import dev.aether.client.design.AetherDesignSystem;
import dev.aether.client.module.HudElement;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Places the HUD elements on the screen.
 *
 * <p>The editor is a workspace, not a page: the elements are the content, so the chrome is pushed to
 * the two edges — one bar at the top, one shelf at the bottom, both separated from the workspace by
 * a hairline and nothing else. No dot grid, no floating card, no accent stripe: the only accent is
 * the guide line shown while an element snaps.</p>
 *
 * <p>The shelf geometry is computed once by {@link #layoutShelf()} and read by both the renderer and
 * the click handler; the previous version recomputed it twice with a slightly different formula.</p>
 */
public class HudEditorScreen extends Screen {
	private static final int BAR_H = 34;
	private static final int PILL_H = 20;
	private static final int PILL_GAP = AetherDesignSystem.Metrics.SPACE_SM;
	private static final int SHELF_PAD = AetherDesignSystem.Metrics.SPACE_SM;
	private static final int ROW_GAP = AetherDesignSystem.Metrics.SPACE_XS;
	private static final int PILL_RADIUS = AetherDesignSystem.Metrics.RADIUS_SM;
	private static final long ENTRANCE_NANOS = AetherDesignSystem.Metrics.ENTER_NANOS;

	private static final int SNAP_DISTANCE = 6;
	private static final int SNAP_MARGIN = 6;

	private final List<HudElement> elements = new ArrayList<>();
	private final List<Pill> pills = new ArrayList<>();

	/**
	 * Positions from before the editor clamped elements into its workspace. A clamp is only there
	 * to keep an element reachable while the editor is open; it must not move the player's HUD
	 * permanently just because the editor was visited.
	 */
	private final Map<HudElement, int[]> positionsBeforeClamp = new LinkedHashMap<>();

	/** One toggle of the bottom shelf, and the element it turns on and off. */
	private record Pill(int x, int y, int w, int h, HudElement element) {
	}

	/** The height of the shelf, computed with its rows: it grows instead of clipping its toggles. */
	private int shelfH = 42;

	/**
	 * The area of the screen the editor can show and grab: everything between the bar and the shelf.
	 * An element left under one of them is painted over and can no longer be picked up, so both the
	 * clamp and the guides read these two edges instead of the edges of the screen.
	 */
	private int workspaceTop;
	private int workspaceBottom;

	private HudElement dragged;

	/** The guide line shown while the dragged element snaps, -1 when none. */
	private int guideX = -1;
	private int guideY = -1;

	/** The screen the editor was opened from. */
	private final Screen previousScreen;
	private final long openedAt = System.nanoTime();
	private float opacity = 1.0f;

	public HudEditorScreen(Screen previousScreen) {
		super(Component.literal("Aether HUD Editor"));
		this.previousScreen = previousScreen;
	}

	@Override
	protected void init() {
		elements.clear();
		ModuleManager manager = Aether.get().getModuleManager();
		for (Module module : manager.getAll()) {
			if (module instanceof HudElement element) {
				elements.add(element);
			}
		}
		layoutShelf();
		// Opening the editor is the moment the chrome gets a size: an element saved under the shelf
		// at another window size is pulled back into the workspace instead of staying out of reach.
		positionsBeforeClamp.clear();
		for (HudElement element : elements) {
			element.updateSize();
			positionsBeforeClamp.put(element, new int[] { element.getX(), element.getY() });
		}
		for (HudElement element : elements) {
			clampIntoWorkspace(element);
		}
	}

	/**
	 * Lays the shelf out in centred rows. A single row is clipped on a narrow window — and a toggle
	 * the player cannot reach is a feature the editor hides — so the shelf wraps and grows instead.
	 */
	private void layoutShelf() {
		pills.clear();
		Font font = Minecraft.getInstance().font;

		List<List<HudElement>> rows = new ArrayList<>();
		List<HudElement> row = new ArrayList<>();
		int rowW = 0;
		for (HudElement element : elements) {
			int w = font.width(element.getName()) + 20;
			if (!row.isEmpty() && rowW + PILL_GAP + w > width - 32) {
				rows.add(row);
				row = new ArrayList<>();
				rowW = 0;
			}
			rowW += (row.isEmpty() ? 0 : PILL_GAP) + w;
			row.add(element);
		}
		if (!row.isEmpty()) {
			rows.add(row);
		}

		shelfH = rows.isEmpty()
			? SHELF_PAD * 2 + PILL_H
			: SHELF_PAD * 2 + rows.size() * PILL_H + (rows.size() - 1) * ROW_GAP;

		workspaceTop = BAR_H + 1;
		workspaceBottom = height - shelfH;

		int y = height - shelfH + SHELF_PAD;
		for (List<HudElement> line : rows) {
			int lineW = PILL_GAP * (line.size() - 1);
			for (HudElement element : line) {
				lineW += font.width(element.getName()) + 20;
			}

			int x = (width - lineW) / 2;
			for (HudElement element : line) {
				int w = font.width(element.getName()) + 20;
				pills.add(new Pill(x, y, w, PILL_H, element));
				x += w + PILL_GAP;
			}
			y += PILL_H + ROW_GAP;
		}
	}

	@Override
	public void render(GuiGraphics graphics, int mouseX, int mouseY, float partialTick) {
		Minecraft minecraft = Minecraft.getInstance();
		Font font = minecraft.font;
		int accent = Aether.get().getConfig().getAccentColor();

		opacity = entrance();
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		RenderUtil.auroraBackdrop(graphics, width, height, System.nanoTime(), 0.96f * opacity);
		drawSurfaceTreatment(graphics, palette);

		for (HudElement element : elements) {
			if (!element.isEnabled()) {
				continue;
			}
			element.updateSize();
			element.render(graphics, partialTick);
			element.renderEditorFrame(graphics);
		}

		int guideColour = AetherDesignSystem.guide(accent);
		if (guideX >= 0) {
			graphics.fill(guideX - 2, 0, guideX + 3, height,
					ColorUtil.withAlpha(accent, 0.10f * opacity));
			graphics.fill(guideX, 0, guideX + 1, height, fade(guideColour));
		}
		if (guideY >= 0) {
			graphics.fill(0, guideY - 2, width, guideY + 3,
					ColorUtil.withAlpha(accent, 0.10f * opacity));
			graphics.fill(0, guideY, width, guideY + 1, fade(guideColour));
		}

		renderBar(graphics, font, accent);
		renderShelf(graphics, font, mouseX, mouseY, accent);
	}

	private void drawSurfaceTreatment(GuiGraphics graphics, AetherDesignSystem.Palette palette) {
		int line = ColorUtil.fade(AetherDesignSystem.backdropPattern(palette), opacity);
		AetherDesignSystem.SurfaceTreatment treatment = AetherDesignSystem.surfaceTreatment(Aether.get().getConfig());
		if (treatment == AetherDesignSystem.SurfaceTreatment.DRAFT) {
			for (int x = 16; x < width; x += 64) graphics.fill(x, BAR_H, x + 1, height - shelfH, line);
			for (int y = BAR_H + 16; y < height - shelfH; y += 64) graphics.fill(0, y, width, y + 1, line);
		} else if (treatment == AetherDesignSystem.SurfaceTreatment.CONTOUR) {
			for (int inset = 28; inset < Math.min(width, height - shelfH) / 2; inset += 38) {
				int w = width - inset * 2, h = height - shelfH - BAR_H - inset * 2;
				if (w > 0 && h > 0) RenderUtil.outline(graphics, inset, BAR_H + inset, w, h, line);
			}
		}
	}

	/** Title on the left, the three gestures on the right, a hairline under both. */
	private void renderBar(GuiGraphics graphics, Font font, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		graphics.fill(0, 0, width, BAR_H, fade(AetherDesignSystem.glass(palette)));
		graphics.fill(0, BAR_H - 1, width, BAR_H, fade(AetherDesignSystem.glassBorder(palette, accent)));
		RenderUtil.roundedRect(graphics, 0, 0, width, 2, 0,
				ColorUtil.fade(AetherDesignSystem.glow(accent, 0.30f), opacity));

		int lift = RenderUtil.enterLift(openedAt);
		RenderUtil.roundedRect(graphics, 16, 10 + lift, 5, 5, 2, fade(accent));
		RenderUtil.text(graphics, font, "HUD EDITOR", 27, 9, tone(AetherDesignSystem.TextRole.TITLE));

		String hint = "Drag to move \u00b7 double-click to reset \u00b7 Esc to return";
		if (width - 32 > font.width("HUD EDITOR") + 24 + font.width(hint)) {
			RenderUtil.text(graphics, font, hint, width - 16 - font.width(hint), 9,
					tone(AetherDesignSystem.TextRole.HINT));
		} else {
			RenderUtil.text(graphics, font, "Esc  return", width - 16 - font.width("Esc  return"), 9,
					tone(AetherDesignSystem.TextRole.HINT));
		}
	}

	private void renderShelf(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		int shelfY = height - shelfH;
		graphics.fill(0, shelfY, width, height, fade(AetherDesignSystem.glass(palette)));
		graphics.fill(0, shelfY, width, shelfY + 1, fade(AetherDesignSystem.glassBorder(palette, accent)));
		RenderUtil.roundedRect(graphics, 0, shelfY, width, 2, 0,
				ColorUtil.fade(AetherDesignSystem.glow(accent, 0.20f), opacity));

		if (pills.isEmpty()) {
			String empty = "No HUD element to place";
			RenderUtil.centeredText(graphics, font, empty, width / 2, shelfY + 16,
					tone(AetherDesignSystem.TextRole.HINT));
			return;
		}

		for (Pill pill : pills) {
			boolean active = pill.element().isEnabled();
			boolean hovered = mouseX >= pill.x() && mouseX < pill.x() + pill.w()
				&& mouseY >= pill.y() && mouseY < pill.y() + pill.h();

			if (active) {
				RenderUtil.roundedRect(graphics, pill.x() - 2, pill.y() - 2, pill.w() + 4, pill.h() + 4,
						PILL_RADIUS + 2, ColorUtil.withAlpha(accent, 0.10f * opacity));
			}
			RenderUtil.bordered(graphics, pill.x(), pill.y(), pill.w(), pill.h(), PILL_RADIUS,
					fade(AetherDesignSystem.chip(palette, accent, active)),
					fade(AetherDesignSystem.chipBorder(palette, accent, active)));

			RenderUtil.centeredText(graphics, font, pill.element().getName(), pill.x() + pill.w() / 2,
					pill.y() + 6, active ? tone(AetherDesignSystem.TextRole.TITLE)
							: tone(AetherDesignSystem.TextRole.LABEL));
		}
	}

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		double mouseX = event.x();
		double mouseY = event.y();

		for (Pill pill : pills) {
			if (mouseX >= pill.x() && mouseX < pill.x() + pill.w()
				&& mouseY >= pill.y() && mouseY < pill.y() + pill.h()) {
				HudElement element = pill.element();
				element.setEnabled(!element.isEnabled());
				if (element.isEnabled()) {
					// An element enabled for the first time has its default position, which can be under
					// the shelf: enabling it has to show it, not hide it behind the chrome.
					clampIntoWorkspace(element);
					persistPosition(element);
				}
				Aether.get().save();
				return true;
			}
		}

		for (HudElement element : elements) {
			if (!element.isEnabled()) {
				continue;
			}
			element.updateSize();
			// The frame that is drawn is the frame that is grabbed, label included.
			if (element.editorFrame().contains(mouseX, mouseY)) {
				if (doubleClick) {
					element.setPosition(SNAP_MARGIN, workspaceTop);
					persistPosition(element);
					return true;
				}
				dragged = element;
				element.startDrag(mouseX, mouseY);
				return true;
			}
		}
		return super.mouseClicked(event, doubleClick);
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		if (dragged != null) {
			// The drop is the moment the position becomes real: it goes into the configuration the
			// client restores, instead of waiting for a clean shutdown that a crash never gives.
			persistPosition(dragged);
			dragged.stopDrag();
			dragged = null;
			guideX = -1;
			guideY = -1;
			Aether.get().save();
			return true;
		}
		return super.mouseReleased(event);
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dragX, double dragY) {
		if (dragged != null) {
			dragged.drag(event.x(), event.y());
			snap(dragged);
			return true;
		}
		return super.mouseDragged(event, dragX, dragY);
	}

	/**
	 * Pulls the element to the nearest edge, centre line or neighbour when it is close enough.
	 */
	private void snap(HudElement element) {
		int x = element.getX();
		int y = element.getY();
		int w = element.getWidth();
		int h = element.getHeight();
		guideX = -1;
		guideY = -1;

		if (Math.abs(x - SNAP_MARGIN) <= SNAP_DISTANCE) {
			x = SNAP_MARGIN;
			guideX = SNAP_MARGIN;
		} else if (Math.abs(x + w / 2 - width / 2) <= SNAP_DISTANCE) {
			x = (width - w) / 2;
			guideX = width / 2;
		} else if (Math.abs(x + w - (width - SNAP_MARGIN)) <= SNAP_DISTANCE) {
			x = width - SNAP_MARGIN - w;
			guideX = width - SNAP_MARGIN;
		}

		// The vertical edges are the edges of the workspace, not of the screen: snapping an element
		// under the shelf used to put it where it could no longer be grabbed.
		if (Math.abs(y - workspaceTop) <= SNAP_DISTANCE) {
			y = workspaceTop;
			guideY = workspaceTop;
		} else if (Math.abs(y + h / 2 - height / 2) <= SNAP_DISTANCE) {
			y = (height - h) / 2;
			guideY = height / 2;
		} else if (Math.abs(y + h - workspaceBottom) <= SNAP_DISTANCE) {
			y = workspaceBottom - h;
			guideY = workspaceBottom;
		}

		// Snapping to other active HUD elements.
		for (HudElement other : elements) {
			if (other == element || !other.isEnabled()) {
				continue;
			}
			int ox = other.getX();
			int oy = other.getY();
			int ow = other.getWidth();
			int oh = other.getHeight();

			if (Math.abs(x - ox) <= SNAP_DISTANCE) {
				x = ox;
				guideX = ox;
			} else if (Math.abs(x - (ox + ow)) <= SNAP_DISTANCE) {
				x = ox + ow + 4;
				guideX = ox + ow;
			} else if (Math.abs((x + w) - ox) <= SNAP_DISTANCE) {
				x = ox - w - 4;
				guideX = ox;
			}

			if (Math.abs(y - oy) <= SNAP_DISTANCE) {
				y = oy;
				guideY = oy;
			} else if (Math.abs(y - (oy + oh)) <= SNAP_DISTANCE) {
				y = oy + oh + 4;
				guideY = oy + oh;
			} else if (Math.abs((y + h) - oy) <= SNAP_DISTANCE) {
				y = oy - h - 4;
				guideY = oy;
			}
		}

		element.setPosition(Math.max(0, Math.min(width - w, x)),
			Math.max(workspaceTop, Math.min(workspaceBottom - h, y)));
	}

	/**
	 * Moves an element back into the workspace, but only when the chrome hides it completely.
	 *
	 * <p>An element the player placed against the top of the screen stays there: the bar covers the
	 * upper part of its label, not the element itself. One that is entirely behind the shelf, on the
	 * other hand, is painted over and answers no click — it is moved to the nearest edge of the
	 * workspace so that enabling it shows it.</p>
	 */
	private void clampIntoWorkspace(HudElement element) {
		element.updateSize();
		int x = Math.max(0, Math.min(Math.max(0, width - element.getWidth()), element.getX()));
		int y = element.getY();
		int h = element.getHeight();
		if (y + h <= workspaceTop) {
			// The editor draws the name above the element: reserve that band so the label is
			// visible under the bar instead of being painted over by it.
			y = freeY(element, x, workspaceTop + HudElement.FRAME_TOP, h, 1);
		} else if (y >= workspaceBottom) {
			y = freeY(element, x, Math.max(workspaceTop + HudElement.FRAME_TOP,
					workspaceBottom - h - HudElement.FRAME_MARGIN), h, -1);
		}
		element.setPosition(x, y);
	}

	/**
	 * Finds the nearest free slot for an element that was hidden by the bar or the shelf.
	 *
	 * <p>Without this, every element saved at the top of the screen was moved to exactly
	 * {@code workspaceTop}: FPS, Ping and CPS landed on top of each other and the player saw only
	 * the last one. The slot is searched away from the chrome, so a row of hidden elements stacks
	 * in reading order instead of disappearing.</p>
	 */
	private int freeY(HudElement element, int x, int y, int h, int direction) {
		int lowest = workspaceTop + HudElement.FRAME_TOP;
		int highest = Math.max(lowest, workspaceBottom - h - HudElement.FRAME_MARGIN);
		for (int attempt = 0; attempt <= elements.size(); attempt++) {
			HudElement.Rect frame = new HudElement.Rect(x - HudElement.FRAME_MARGIN,
					y - HudElement.FRAME_TOP, element.getWidth() + HudElement.FRAME_MARGIN * 2,
					h + HudElement.FRAME_MARGIN * 2 + (HudElement.FRAME_TOP - HudElement.FRAME_MARGIN));
			HudElement hit = null;
			for (HudElement other : elements) {
				if (other == element || !other.isEnabled()) {
					continue;
				}
				HudElement.Rect otherFrame = other.editorFrame();
				boolean overlapX = frame.x() < otherFrame.x() + otherFrame.w() + 2
						&& frame.x() + frame.w() + 2 > otherFrame.x();
				boolean overlapY = frame.y() < otherFrame.y() + otherFrame.h() + 2
						&& frame.y() + frame.h() + 2 > otherFrame.y();
				if (overlapX && overlapY) {
					hit = other;
					break;
				}
			}
			if (hit == null) {
				break;
			}
			y = direction > 0
					? hit.editorFrame().y() + hit.editorFrame().h() + 2 + HudElement.FRAME_TOP
					: hit.editorFrame().y() - 2 - h - HudElement.FRAME_MARGIN;
			y = Math.max(lowest, Math.min(highest, y));
		}
		return y;
	}

	/**
	 * Writes the position of one element to the configuration file, now.
	 */
	private void persistPosition(HudElement element) {
		// A position the player chose (drag, snap or enabling a chip) is not restored on close.
		positionsBeforeClamp.remove(element);
		Aether aether = Aether.get();
		aether.getConfig().hudPositions.put(element.getName().toLowerCase(Locale.ROOT),
				new int[] { element.getX(), element.getY() });
		aether.save();
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (event.key() == 256) {
			onClose();
			return true;
		}
		return super.keyPressed(event);
	}

	@Override
	public void onClose() {
		// Undo the clamp used to make the top and bottom elements reachable in the editor. Without
		// this, simply opening the HUD editor moved FPS and Ping under the coordinates panel, and
		// the next shutdown persisted those accidental positions.
		for (Map.Entry<HudElement, int[]> entry : positionsBeforeClamp.entrySet()) {
			entry.getKey().setPosition(entry.getValue()[0], entry.getValue()[1]);
		}
		positionsBeforeClamp.clear();
		Aether.get().save();
		Minecraft.getInstance().setScreen(previousScreen);
	}

	@Override
	public boolean isPauseScreen() {
		return false;
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

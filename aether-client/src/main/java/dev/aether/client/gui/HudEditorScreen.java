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
import java.util.List;
import java.util.Locale;

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
	private static final int PILL_GAP = 6;
	private static final int SHELF_PAD = 8;
	private static final int ROW_GAP = 4;

	private static final int SNAP_DISTANCE = 6;
	private static final int SNAP_MARGIN = 6;

	private final List<HudElement> elements = new ArrayList<>();
	private final List<Pill> pills = new ArrayList<>();

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

		AetherDesignSystem.Palette palette = RenderUtil.theme();
		graphics.fill(0, 0, width, height, ColorUtil.withAlpha(palette.background(), 0.82f));
		drawSurfaceTreatment(graphics, palette);

		for (HudElement element : elements) {
			if (!element.isEnabled()) {
				continue;
			}
			element.updateSize();
			element.render(graphics, partialTick);
			element.renderEditorFrame(graphics);
		}

		int guideColour = ColorUtil.withAlpha(accent, 0.65f);
		if (guideX >= 0) {
			graphics.fill(guideX, 0, guideX + 1, height, guideColour);
		}
		if (guideY >= 0) {
			graphics.fill(0, guideY, width, guideY + 1, guideColour);
		}

		renderBar(graphics, font);
		renderShelf(graphics, font, mouseX, mouseY, accent);
	}

	private void drawSurfaceTreatment(GuiGraphics graphics, AetherDesignSystem.Palette palette) {
		int line = ColorUtil.withAlpha(palette.borderStrong(), 0.14f);
		AetherDesignSystem.SurfaceTreatment treatment = AetherDesignSystem.surfaceTreatment(Aether.get().getConfig());
		if (treatment == AetherDesignSystem.SurfaceTreatment.DRAFT) {
			for (int x=16; x<width; x+=64) graphics.fill(x,BAR_H,x+1,height-shelfH,line);
			for (int y=BAR_H+16; y<height-shelfH; y+=64) graphics.fill(0,y,width,y+1,line);
		} else if (treatment == AetherDesignSystem.SurfaceTreatment.CONTOUR) {
			for (int inset=28; inset<Math.min(width,height-shelfH)/2; inset+=38) {
				int w=width-inset*2, h=height-shelfH-BAR_H-inset*2;
				if(w>0&&h>0) RenderUtil.outline(graphics,inset,BAR_H+inset,w,h,line);
			}
		}
	}

	/** Title on the left, the three gestures on the right, a hairline under both. */
	private void renderBar(GuiGraphics graphics, Font font) {
		graphics.fill(0, 0, width, BAR_H, RenderUtil.theme().elevatedBackground());
		graphics.fill(0, BAR_H - 1, width, BAR_H, RenderUtil.theme().border());

		RenderUtil.text(graphics, font, "HUD editor", 16, 12, RenderUtil.theme().textPrimary());

		String hint = "Drag to move \u00b7 double-click to reset \u00b7 Esc to return";
		RenderUtil.text(graphics, font, hint, width - 16 - font.width(hint), 12, RenderUtil.theme().textDim());
	}

	private void renderShelf(GuiGraphics graphics, Font font, int mouseX, int mouseY, int accent) {
		int shelfY = height - shelfH;
		graphics.fill(0, shelfY, width, height, RenderUtil.theme().elevatedBackground());
		graphics.fill(0, shelfY, width, shelfY + 1, RenderUtil.theme().border());

		if (pills.isEmpty()) {
			String empty = "No HUD element to place";
			RenderUtil.centeredText(graphics, font, empty, width / 2, shelfY + 16, RenderUtil.theme().textDim());
			return;
		}

		for (Pill pill : pills) {
			boolean active = pill.element().isEnabled();
			boolean hovered = mouseX >= pill.x() && mouseX < pill.x() + pill.w()
				&& mouseY >= pill.y() && mouseY < pill.y() + pill.h();

			// An enabled element is a surface, not a tint of the accent: the accent stays on the
			// border, where it is a state marker and not a background colour.
			// `bordered` paints its border as a full rectangle, so the fill is always opaque: a
			// transparent fill would leave a solid slab of border colour where an empty chip should be.
			int fill = active
				? (hovered ? RenderUtil.theme().surfaceHover() : RenderUtil.theme().surfaceActive())
				: (hovered ? RenderUtil.theme().surfaceHover() : RenderUtil.theme().surface());
			RenderUtil.bordered(graphics, pill.x(), pill.y(), pill.w(), pill.h(), 4, fill,
				active ? accent : RenderUtil.theme().border());

			int textColour = active ? RenderUtil.theme().textPrimary() : RenderUtil.theme().textSecondary();
			RenderUtil.centeredText(graphics, font, pill.element().getName(), pill.x() + pill.w() / 2,
				pill.y() + 6, textColour);
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
		if (y + element.getHeight() <= workspaceTop) {
			y = workspaceTop;
		} else if (y >= workspaceBottom) {
			y = Math.max(workspaceTop, workspaceBottom - element.getHeight());
		}
		element.setPosition(x, y);
	}

	/**
	 * Writes the position of one element to the configuration file, now.
	 */
	private void persistPosition(HudElement element) {
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
		Aether.get().save();
		Minecraft.getInstance().setScreen(previousScreen);
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}
}

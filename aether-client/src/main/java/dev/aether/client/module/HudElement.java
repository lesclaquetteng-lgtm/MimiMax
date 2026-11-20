package dev.aether.client.module;

import dev.aether.client.Aether;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;

/**
 * A module that is drawn on the HUD.
 *
 * HUD elements can be moved around with the HUD editor and remember their position.
 */
public abstract class HudElement extends Module {
	/** The margin the editor draws around an element, and therefore the margin the player can grab. */
	public static final int FRAME_MARGIN = 3;
	/** The height of the band that carries the name of the element above its frame. */
	private static final int LABEL_H = 11;

	private int x;
	private int y;
	private int width;
	private int height;

	private boolean dragging;
	private int dragOffsetX;
	private int dragOffsetY;

	protected HudElement(String name, String description, ModuleCategory category, boolean defaultEnabled) {
		super(name, description, category, defaultEnabled);
	}

	/**
	 * Draws the element at the given position.
	 */
	public void render(GuiGraphics graphics, float partialTick) {
		onRender(graphics, partialTick);
	}

	/**
	 * @return the width the element wants to use.
	 */
	public abstract int measureWidth();

	/**
	 * @return the height the element wants to use.
	 */
	public abstract int measureHeight();

	/**
	 * Called after the element was rendered, used to measure it.
	 */
	public void updateSize() {
		this.width = measureWidth();
		this.height = measureHeight();
	}

	public int getX() {
		return x;
	}

	public int getY() {
		return y;
	}

	public void setPosition(int x, int y) {
		this.x = x;
		this.y = y;
	}

	public int getWidth() {
		return width;
	}

	public int getHeight() {
		return height;
	}

	/** One rectangle of the HUD editor, with the hit test that goes with it. */
	public record Rect(int x, int y, int w, int h) {
		public boolean contains(double mouseX, double mouseY) {
			return mouseX >= x && mouseX < x + w && mouseY >= y && mouseY < y + h;
		}
	}

	/**
	 * The frame the editor draws around the element: the element, its margin and the label above it.
	 *
	 * <p>One method, not two. The renderer used to draw a frame slightly wider than the element
	 * while the editor tested the element alone, so the label of an element and the margin around
	 * it answered nothing: the player aimed at what they saw and nothing moved.</p>
	 *
	 * @return the rectangle both the renderer and the input handler of the editor read.
	 */
	public Rect editorFrame() {
		int labelWidth = labelWidth();
		return new Rect(x - FRAME_MARGIN, y - FRAME_MARGIN - LABEL_H,
			Math.max(getWidth() + FRAME_MARGIN * 2, labelWidth),
			getHeight() + FRAME_MARGIN * 2 + LABEL_H);
	}

	/**
	 * @return the width of the label the editor draws above the element.
	 */
	private int labelWidth() {
		return Minecraft.getInstance().font.width(getName()) + 14;
	}

	public void startDrag(double mouseX, double mouseY) {
		dragging = true;
		dragOffsetX = (int) mouseX - x;
		dragOffsetY = (int) mouseY - y;
	}

	public void stopDrag() {
		dragging = false;
	}

	public void drag(double mouseX, double mouseY) {
		if (!dragging) {
			return;
		}
		x = (int) mouseX - dragOffsetX;
		y = (int) mouseY - dragOffsetY;
	}

	/**
	 * Draws the frame that surrounds the element while the HUD editor is open.
	 *
	 * The frame carries the name of the element, so that the editor does not need a second list.
	 */
	public void renderEditorFrame(GuiGraphics graphics) {
		Aether aether = Aether.getOrNull();
		int accent = aether == null ? 0xFFE7E9EC : aether.getConfig().getAccentColor();
		Font font = Minecraft.getInstance().font;
		int labelWidth = labelWidth();
		Rect frame = editorFrame();

		// An outline, not a veil: the element under it stays readable while it is being placed, and
		// the frame costs one pixel instead of painting a translucent box over the HUD.
		RenderUtil.outline(graphics, frame.x(), frame.y() + LABEL_H, frame.w(),
				frame.h() - LABEL_H, ColorUtil.withAlpha(accent, 0.6f));
		RenderUtil.roundedRect(graphics, frame.x(), frame.y(), labelWidth, LABEL_H, 3, accent);
		RenderUtil.text(graphics, font, getName(), frame.x() + 4, frame.y() + 2, RenderUtil.theme().onAccentText(), false);
	}
}

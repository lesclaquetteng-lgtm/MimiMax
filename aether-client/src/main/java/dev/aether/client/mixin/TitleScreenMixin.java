package dev.aether.client.mixin;

import dev.aether.client.Aether;
import dev.aether.client.AetherClient;
import dev.aether.client.design.AetherDesignSystem;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.util.SessionUtil;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.TitleScreen;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

import java.util.ArrayList;
import java.util.List;

/**
 * Replaces the background of the main menu with the design of the client and shows the state of
 * the session.
 */
@Mixin(TitleScreen.class)
public abstract class TitleScreenMixin {

	/**
	 * Draws the background of the client instead of the panorama of the game.
	 */
	@Inject(method = "renderBackground", at = @At("HEAD"), cancellable = true)
	private void aether$renderBackground(GuiGraphics graphics, int mouseX, int mouseY, float partialTick,
			CallbackInfo callback) {
		int width = graphics.guiWidth();
		int height = graphics.guiHeight();
		AetherDesignSystem.Palette palette = RenderUtil.theme();
		int accent = palette.accent();
		graphics.fillGradient(0, 0, width, height / 2, palette.elevatedBackground(), palette.background());
		graphics.fillGradient(0, height / 2, width, height, palette.background(), palette.background());

		// Cinematic micro-stars
		int starCol = ColorUtil.withAlpha(0xFFFFFF, 0.15f);
		for (int i = 0; i < 48; i++) {
			int sx = (i * 127 + 23) % width;
			int sy = (i * 163 + 71) % height;
			graphics.fill(sx, sy, sx + 1, sy + 1, starCol);
		}

		// Atmospheric horizon glow in the centre
		int horizonY = height / 2;
		graphics.fillGradient(0, horizonY - 50, width, horizonY, 0x00000000, ColorUtil.withAlpha(accent, 0.08f));
		graphics.fillGradient(0, horizonY, width, horizonY + 60, ColorUtil.withAlpha(accent, 0.08f), 0x00000000);

		// Top edge subtle accent line
		graphics.fillGradient(0, 0, width, 2, accent, ColorUtil.withAlpha(accent, 0.3f));

		callback.cancel();
	}

	/**
	 * Draws the branding of the client and the state of the session.
	 */
	@Inject(method = "render", at = @At("HEAD"))
	private void aether$renderBranding(GuiGraphics graphics, int mouseX, int mouseY, float partialTick,
			CallbackInfo callback) {
		if (Aether.getOrNull() == null) {
			return;
		}

		int accent = Aether.get().getConfig().getAccentColor();
		var font = Minecraft.getInstance().font;

		// Floating Idle Animated Logo & Pulsing Glow
		long time = System.currentTimeMillis();
		double floatOffset = Math.sin(time / 450.0) * 3.0;
		float pulseAlpha = (float) (0.20 + 0.15 * Math.sin(time / 300.0));
		int brandY = (int) (12 + floatOffset);

		int brandW = 140;
		RenderUtil.card(graphics, 12, brandY, brandW, 36, 6);
		// Pulsing glowing crest
		RenderUtil.roundedRect(graphics, 16, brandY + 4, 28, 28, 6, ColorUtil.withAlpha(accent, pulseAlpha));
		RenderUtil.roundedRect(graphics, 18, brandY + 6, 24, 24, 4, accent);
		RenderUtil.centeredText(graphics, font, "A", 30, brandY + 12, 0xFFFFFFFF);
		RenderUtil.text(graphics, font, "AETHER", 48, brandY + 6, 0xFFFFFFFF);
		RenderUtil.text(graphics, font, "Client v" + Aether.VERSION, 48, brandY + 17, ColorUtil.blend(accent, 0xFFFFFFFF, 0.5f));

		// Interactive animated button underline on hover
		TitleScreen titleScreen = (TitleScreen) (Object) this;
		for (var child : titleScreen.children()) {
			if (child instanceof AbstractWidget widget && widget.visible && widget.active) {
				if (widget.isHovered()) {
					int lineY = widget.getY() + widget.getHeight() - 1;
					int lineWidth = (int) (widget.getWidth() * 0.85);
					int lineX = widget.getX() + (widget.getWidth() - lineWidth) / 2;
					graphics.fill(lineX, lineY, lineX + lineWidth, lineY + 2, accent);
				}
			}
		}

		// The state of the session in top right corner.
		boolean offline = SessionUtil.isOffline();
		String label = offline ? "OFFLINE SESSION" : "MICROSOFT ACCOUNT";
		int labelWidth = font.width(label) + 20;
		int labelX = graphics.guiWidth() - labelWidth - 12;
		int statusColor = offline ? 0xFFFFD60A : 0xFF34C759;

		RenderUtil.card(graphics, labelX, 12, labelWidth, 22, 5);
		RenderUtil.roundedRect(graphics, labelX + 6, 19, 6, 6, 3, statusColor);
		RenderUtil.text(graphics, font, label, labelX + 16, 17, statusColor);

		// A floating hint that shows how the click GUI is opened. The label follows the binding the
		// player actually chose instead of advertising the default key after a remap.
		AetherClient client = AetherClient.getInstance();
		String hintKey = "[" + (client == null ? "R-SHIFT" : client.getClickGuiKeyLabel()) + "]";
		String hintText = "Open Click GUI";
		int keyW = font.width(hintKey) + 6;
		int totalW = keyW + font.width(hintText) + 18;
		int hintY = graphics.guiHeight() - 26;
		RenderUtil.card(graphics, 12, hintY, totalW, 18, 4);
		RenderUtil.roundedRect(graphics, 16, hintY + 3, keyW, 12, 3, accent);
		RenderUtil.text(graphics, font, hintKey, 19, hintY + 5, 0xFFFFFFFF);
		RenderUtil.text(graphics, font, hintText, 16 + keyW + 6, hintY + 5, RenderUtil.theme().textPrimary());
	}

	/**
	 * Hides the realms button when the player uses an offline account, because the realms can
	 * only be used with a premium account.
	 */
	@Inject(method = "init", at = @At("TAIL"))
	private void aether$hideRealms(CallbackInfo callback) {
		if (Aether.getOrNull() == null || !SessionUtil.isOffline()) {
			return;
		}

		TitleScreen screen = (TitleScreen) (Object) this;
		List<net.minecraft.client.gui.components.events.GuiEventListener> children =
				new ArrayList<>(screen.children());

		for (net.minecraft.client.gui.components.events.GuiEventListener child : children) {
			if (child instanceof AbstractWidget widget) {
				String message = widget.getMessage().getString();
				if (message.contains("Realms")) {
					widget.visible = false;
					widget.active = false;
				}
			}
		}
	}
}

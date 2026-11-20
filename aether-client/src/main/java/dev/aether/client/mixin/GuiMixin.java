package dev.aether.client.mixin;

import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleKeys;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiGraphics;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Lets the client replace the crosshair of the game and hide the scoreboard.
 */
@Mixin(Gui.class)
public abstract class GuiMixin {

	/**
	 * Hides the crosshair of the game while the custom crosshair is enabled.
	 */
	@Inject(method = "renderCrosshair", at = @At("HEAD"), cancellable = true)
	private void aether$hideCrosshair(GuiGraphics graphics, DeltaTracker deltaTracker, CallbackInfo callback) {
		if (Aether.getOrNull() == null) {
			return;
		}
		Module module = Aether.get().getModuleManager().get(ModuleKeys.CROSSHAIR);
		if (module != null && module.isEnabled()) {
			callback.cancel();
		}
	}

	/**
	 * Hides the scoreboard of the server while the module is enabled.
	 */
	@Inject(method = "renderScoreboardSidebar", at = @At("HEAD"), cancellable = true)
	private void aether$hideScoreboard(GuiGraphics graphics, DeltaTracker deltaTracker, CallbackInfo callback) {
		if (Aether.getOrNull() == null) {
			return;
		}
		Module module = Aether.get().getModuleManager().get(ModuleKeys.HIDE_SCOREBOARD);
		if (module != null && module.isEnabled()) {
			callback.cancel();
		}
	}
}

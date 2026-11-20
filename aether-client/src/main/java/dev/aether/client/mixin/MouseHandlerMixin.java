package dev.aether.client.mixin;

import dev.aether.client.Aether;
import dev.aether.client.module.modules.HudModules;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.input.MouseButtonInfo;
import net.minecraft.world.phys.HitResult;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Records every mouse click so that the client can show the clicks per second.
 */
@Mixin(MouseHandler.class)
public abstract class MouseHandlerMixin {

	/**
	 * Called by the game whenever a mouse button is pressed or released.
	 *
	 * @param window the identifier of the window
	 * @param info   the button that was used
	 * @param action the action, {@code 1} means the button was pressed
	 */
	@Inject(method = "onButton", at = @At("HEAD"))
	private void aether$onButton(long window, MouseButtonInfo info, int action, CallbackInfo callback) {
		if (action != 1) {
			return;
		}
		if (Aether.getOrNull() == null) {
			return;
		}
		Aether.get().getClickTracker().record(info.button());
		if (info.button() == 0) {
			countHit();
		}
	}

	/**
	 * Feeds the combo counter of the HUD: a left click inside the game, with the crosshair on an
	 * entity, is a hit.
	 *
	 * <p>The counter declared a {@code recordHit()} that nothing ever called, so the element sat at
	 * “×0” for a whole session.</p>
	 */
	private static void countHit() {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft == null || minecraft.screen != null || minecraft.player == null) {
			return;
		}
		if (minecraft.hitResult != null && minecraft.hitResult.getType() == HitResult.Type.ENTITY) {
			HudModules.ComboCounter.recordHit();
		}
	}
}

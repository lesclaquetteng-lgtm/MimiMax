package dev.aether.client.mixin;

import com.mojang.blaze3d.platform.InputConstants;
import dev.aether.client.gui.ClickGuiScreen;
import dev.aether.client.module.ModuleCategory;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.input.KeyEvent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Makes the click GUI reachable from the main menu.
 *
 * Minecraft only registers the keybinds while no screen is open, the click GUI therefore has to be
 * opened by the screen itself.
 */
@Mixin(Screen.class)
public abstract class ScreenMixin {

	/**
	 * Opens the click GUI when the right shift key is pressed on the main menu.
	 */
	@Inject(method = "keyPressed", at = @At("HEAD"), cancellable = true)
	private void aether$keyPressed(KeyEvent event, CallbackInfoReturnable<Boolean> callback) {
		if (event.key() != InputConstants.KEY_RSHIFT) {
			return;
		}
		if (!((Object) this instanceof TitleScreen)) {
			return;
		}

		Minecraft minecraft = Minecraft.getInstance();
		minecraft.setScreen(new ClickGuiScreen(ModuleCategory.RENDER, (TitleScreen) (Object) this));
		callback.setReturnValue(true);
	}
}

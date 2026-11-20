package dev.aether.client.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleKeys;
import dev.aether.client.module.modules.RenderModules;
import net.minecraft.client.Camera;
import net.minecraft.client.renderer.GameRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Hooks the two camera values the client changes: the damage tilt and the field of view.
 *
 * <p>Both used to be written into the options of the game. Options are saved in {@code options.txt},
 * so a module that wrote its own value there could not tell, on the next launch, what the value had
 * been before it: disabling the module then restored the value the module itself had written. These
 * injections change the computed value for one frame and leave the options alone.</p>
 */
@Mixin(GameRenderer.class)
public abstract class GameRendererMixin {

	/**
	 * Skips the damage tilt while the module is enabled.
	 *
	 * <p>Minecraft 1.21 draws no red texture over the screen for a hit: {@code bobHurt} is the whole
	 * feedback, and cancelling it is the only way to remove it without writing into an option.</p>
	 */
	@Inject(method = "bobHurt(Lcom/mojang/blaze3d/vertex/PoseStack;F)V",
			at = @At("HEAD"), cancellable = true)
	private void aether$hideHurtTilt(PoseStack poseStack, float partialTick, CallbackInfo callback) {
		if (Aether.getOrNull() == null) {
			return;
		}
		Module module = Aether.get().getModuleManager().get(ModuleKeys.HIDE_HURT_OVERLAY);
		if (module != null && module.isEnabled()) {
			callback.cancel();
		}
	}

	/**
	 * Divides the field of view while the zoom is enabled.
	 *
	 * <p>The hook is at the return of {@code getFov}, so it also applies to the field of view of a
	 * spyglass or of a panorama, and it leaves the value the player chose in the options intact.</p>
	 */
	@Inject(method = "getFov(Lnet/minecraft/client/Camera;FZ)F",
			at = @At("RETURN"), cancellable = true)
	private void aether$zoom(Camera camera, float partialTick, boolean useFovSetting,
			CallbackInfoReturnable<Float> callback) {
		if (Aether.getOrNull() == null) {
			return;
		}
		Module module = Aether.get().getModuleManager().get(ModuleKeys.ZOOM);
		if (module instanceof RenderModules.Zoom zoom && zoom.isEnabled()) {
			callback.setReturnValue((float) (callback.getReturnValueF() / zoom.getZoomFactor()));
		}
	}
}

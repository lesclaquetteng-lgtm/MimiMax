package dev.aether.client.mixin;

import com.mojang.blaze3d.vertex.PoseStack;
import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleKeys;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.ScreenEffectRenderer;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Lets the client hide the fire overlay of the screen.
 *
 * <p>The overlay is drawn by {@code ScreenEffectRenderer#renderFire}, and only that method: the
 * block and water overlays go through their own, so cancelling this one hides the flames without
 * touching what the player sees inside a wall or underwater.</p>
 */
@Mixin(ScreenEffectRenderer.class)
public abstract class ScreenEffectRendererMixin {

	/**
	 * Skips the flames drawn over the screen while the module is enabled.
	 */
	@Inject(method = "renderFire(Lcom/mojang/blaze3d/vertex/PoseStack;"
			+ "Lnet/minecraft/client/renderer/MultiBufferSource;"
			+ "Lnet/minecraft/client/renderer/texture/TextureAtlasSprite;)V",
			at = @At("HEAD"), cancellable = true)
	private static void aether$hideFire(PoseStack poseStack, MultiBufferSource bufferSource,
			TextureAtlasSprite sprite, CallbackInfo callback) {
		if (Aether.getOrNull() == null) {
			return;
		}
		Module module = Aether.get().getModuleManager().get(ModuleKeys.HIDE_FIRE_OVERLAY);
		if (module != null && module.isEnabled()) {
			callback.cancel();
		}
	}
}

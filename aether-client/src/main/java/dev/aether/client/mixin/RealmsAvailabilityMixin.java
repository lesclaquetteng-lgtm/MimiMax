package dev.aether.client.mixin;

import com.mojang.realmsclient.RealmsAvailability;
import dev.aether.client.util.SessionUtil;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.concurrent.CompletableFuture;

/**
 * Avoids sending an offline placeholder token to Realms just to learn that it is not authenticated.
 * Offline players cannot use Realms, and the title screen already hides its Realms entry for them.
 */
@Mixin(RealmsAvailability.class)
public abstract class RealmsAvailabilityMixin {
	@Inject(method = "check", at = @At("HEAD"), cancellable = true)
	private static void aether$skipOfflineAvailabilityCheck(
			CallbackInfoReturnable<CompletableFuture<RealmsAvailability.Result>> callback) {
		if (SessionUtil.isOffline()) {
			callback.setReturnValue(CompletableFuture.completedFuture(
					new RealmsAvailability.Result(RealmsAvailability.Type.AUTHENTICATION_ERROR)));
		}
	}
}

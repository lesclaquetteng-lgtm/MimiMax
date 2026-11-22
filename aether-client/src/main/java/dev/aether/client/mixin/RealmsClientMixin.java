package dev.aether.client.mixin;

import com.mojang.realmsclient.client.RealmsClient;
import dev.aether.client.util.SessionUtil;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.Collections;
import java.util.Set;

/**
 * Realms feature flags are only useful to authenticated players. Returning none for offline sessions
 * keeps the background menu fetch from trying to parse the launcher's placeholder access token.
 */
@Mixin(RealmsClient.class)
public abstract class RealmsClientMixin {
	@Inject(method = "fetchFeatureFlags", at = @At("HEAD"), cancellable = true)
	private void aether$skipOfflineFeatureFlagsFetch(CallbackInfoReturnable<Set<String>> callback) {
		if (SessionUtil.isOffline()) {
			callback.setReturnValue(Collections.emptySet());
		}
	}
}

package dev.aether.client.mixin;

import com.mojang.authlib.minecraft.UserApiService;
import com.mojang.authlib.yggdrasil.YggdrasilAuthenticationService;
import net.minecraft.client.Minecraft;
import net.minecraft.client.main.GameConfig;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.UUID;

/**
 * Keeps offline sessions away from the Mojang servers.
 *
 * The game normally asks Mojang for the properties of the profile as soon as it starts. With an
 * offline account that request can only fail, it produces an error in the log and slows the start
 * down. Authlib already ships an implementation that does not talk to the network at all, it is
 * used instead as soon as the session is recognised as an offline one.
 */
@Mixin(Minecraft.class)
public abstract class MinecraftMixin {

	/**
	 * Replaces the service of the user with the offline implementation when the session is an
	 * offline one.
	 */
	@Inject(method = "createUserApiService", at = @At("HEAD"), cancellable = true)
	private void aether$offlineUserApiService(YggdrasilAuthenticationService service, GameConfig config,
			CallbackInfoReturnable<UserApiService> callback) {
		if (config == null || config.user == null || config.user.user == null) {
			return;
		}

		var user = config.user.user;
		String token = user.getAccessToken();
		boolean placeholderToken = token == null || token.isEmpty() || token.equals("0");
		boolean offlineIdentifier = isOfflineIdentifier(user.getName(), user.getProfileId());

		if (placeholderToken || offlineIdentifier) {
			callback.setReturnValue(UserApiService.OFFLINE);
		}
	}

	/**
	 * @return true when the identifier is the one Minecraft derives from the name.
	 */
	private static boolean isOfflineIdentifier(String name, UUID identifier) {
		if (name == null || identifier == null) {
			return false;
		}
		UUID expected = UUID.nameUUIDFromBytes(("OfflinePlayer:" + name)
				.getBytes(java.nio.charset.StandardCharsets.UTF_8));
		return expected.equals(identifier);
	}
}

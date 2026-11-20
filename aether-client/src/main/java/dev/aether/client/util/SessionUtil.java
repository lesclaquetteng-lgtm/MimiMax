package dev.aether.client.util;

import net.minecraft.client.Minecraft;

import java.nio.charset.StandardCharsets;
import java.util.UUID;

/**
 * Information about the session the game is running with.
 *
 * The launcher starts the game with a placeholder access token when the player uses an offline
 * account. The helper detects this so that the interface can show the correct state.
 */
public final class SessionUtil {
	private SessionUtil() {
	}

	/**
	 * @return the name of the player.
	 */
	public static String getUsername() {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft == null || minecraft.getUser() == null) {
			return "Player";
		}
		return minecraft.getUser().getName();
	}

	/**
	 * @return the identifier of the player.
	 */
	public static UUID getUuid() {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft == null || minecraft.getUser() == null) {
			return UUID.nameUUIDFromBytes("OfflinePlayer:Player".getBytes(StandardCharsets.UTF_8));
		}
		return minecraft.getUser().getProfileId();
	}

	/**
	 * @return the access token of the session.
	 */
	public static String getAccessToken() {
		Minecraft minecraft = Minecraft.getInstance();
		if (minecraft == null || minecraft.getUser() == null) {
			return "";
		}
		return minecraft.getUser().getAccessToken();
	}

	/**
	 * @return true when the game runs with an offline session.
	 *
	 * An offline session either uses the placeholder token or the identifier Minecraft derives
	 * from the player name.
	 */
	public static boolean isOffline() {
		String token = getAccessToken();
		if (token == null || token.isEmpty() || token.equals("0")) {
			return true;
		}
		return getUuid().equals(offlineUuid(getUsername()));
	}

	/**
	 * Computes the identifier Minecraft assigns to an offline player.
	 */
	public static UUID offlineUuid(String username) {
		return UUID.nameUUIDFromBytes(("OfflinePlayer:" + username).getBytes(StandardCharsets.UTF_8));
	}

	/**
	 * @return a short description of the session, shown in the interface.
	 */
	public static String getSessionLabel() {
		return isOffline() ? "Offline" : "Microsoft";
	}
}

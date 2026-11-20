package dev.aether.client;

import dev.aether.client.util.SessionUtil;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.UUID;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotEquals;

/**
 * Verifies that the offline identifiers of the client are the same as the ones Minecraft computes.
 */
class OfflineSessionTest {

	// Produced with `java.util.UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`, which is what
	// `net.minecraft.util.Util#generateOfflinePlayerUuid` does inside the game.
	private static final Object[][] REFERENCE = {
			{ "Notch", "b50ad385-829d-3141-a216-7e7d7539ba7f" },
			{ "Steve", "5627dd98-e6be-3c21-b8a8-e92344183641" },
			{ "Player123", "3da1e349-17b0-38ed-a07b-1e78c8ebd956" },
			{ "aether_test", "192d7399-e38b-3155-afd3-d30d29bf7d22" }
	};

	@Test
	@DisplayName("the offline identifier matches the reference implementation of Minecraft")
	void offlineIdentifierMatchesMinecraft() {
		for (Object[] entry : REFERENCE) {
			String name = (String) entry[0];
			String expected = (String) entry[1];
			assertEquals(expected, SessionUtil.offlineUuid(name).toString(), "uuid mismatch for " + name);
		}
	}

	@Test
	@DisplayName("the offline identifier is a version 3 uuid")
	void offlineIdentifierIsVersionThree() {
		UUID uuid = SessionUtil.offlineUuid("Notch");
		assertEquals(3, uuid.version());
		assertEquals(2, uuid.variant());
	}

	@Test
	@DisplayName("the offline identifier only depends on the name")
	void offlineIdentifierIsStable() {
		assertEquals(SessionUtil.offlineUuid("Steve"), SessionUtil.offlineUuid("Steve"));
		assertNotEquals(SessionUtil.offlineUuid("Steve"), SessionUtil.offlineUuid("steve"));
	}
}

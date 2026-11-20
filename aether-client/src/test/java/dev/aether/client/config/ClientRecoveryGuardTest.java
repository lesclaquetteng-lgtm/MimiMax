package dev.aether.client.config;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Regression tests for the non-destructive abnormal-shutdown marker. */
class ClientRecoveryGuardTest {
	@Test
	@DisplayName("a safe-mode recommendation appears only after repeated unclean starts")
	void repeatedUncleanStartsRecommendRecovery(@TempDir Path directory) throws Exception {
		Path marker = directory.resolve("session.guard");

		ClientRecoveryGuard.Startup first = ClientRecoveryGuard.begin(marker);
		assertEquals(0, first.uncleanStartStreak());
		assertFalse(first.recoveryRecommended());

		// Simulate the process exiting without reaching ClientLifecycleEvents.CLIENT_STOPPING.
		ClientRecoveryGuard.Startup second = ClientRecoveryGuard.begin(marker);
		assertEquals(1, second.uncleanStartStreak());
		assertFalse(second.recoveryRecommended());

		// A second consecutive abnormal exit crosses the recovery threshold.
		ClientRecoveryGuard.Startup third = ClientRecoveryGuard.begin(marker);
		assertEquals(2, third.uncleanStartStreak());
		assertTrue(third.recoveryRecommended());
	}

	@Test
	@DisplayName("an orderly shutdown clears the streak")
	void cleanShutdownResetsTheStreak(@TempDir Path directory) throws Exception {
		Path marker = directory.resolve("session.guard");
		ClientRecoveryGuard.begin(marker);
		ClientRecoveryGuard.begin(marker);
		assertTrue(Files.exists(marker));

		ClientRecoveryGuard.finish(marker);

		assertFalse(Files.exists(marker));
		ClientRecoveryGuard.Startup next = ClientRecoveryGuard.begin(marker);
		assertEquals(0, next.uncleanStartStreak());
		assertFalse(next.recoveryRecommended());
	}

	@Test
	@DisplayName("a malformed marker is preserved and does not invent a crash count")
	void corruptMarkerIsPreserved(@TempDir Path directory) throws Exception {
		Path marker = directory.resolve("session.guard");
		Files.writeString(marker, "not-a-number", StandardCharsets.UTF_8);

		ClientRecoveryGuard.Startup startup = ClientRecoveryGuard.begin(marker);

		assertEquals(0, startup.uncleanStartStreak());
		assertFalse(startup.recoveryRecommended());
		assertEquals("0", Files.readString(marker, StandardCharsets.UTF_8).trim());
		try (var files = Files.list(directory)) {
			assertTrue(files.anyMatch(path -> path.getFileName().toString().startsWith("session.guard.corrupt-")));
		}
	}
}

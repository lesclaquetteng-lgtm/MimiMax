package dev.aether.client.config;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Persistence and recovery tests for the client configuration. */
class AetherConfigRecoveryTest {
	@Test
	@DisplayName("a failed final replacement leaves the last valid primary untouched")
	void failedReplacementDoesNotDamageThePrimary(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		AetherConfig original = new AetherConfig();
		original.accentColor = "#2ECC8F";
		original.save(file);
		String originalBytes = Files.readString(file, StandardCharsets.UTF_8);

		AetherConfig changed = new AetherConfig();
		changed.accentColor = "#D99A69";
		changed.save(file, (staged, target) -> {
			throw new IOException("simulated interruption before atomic promotion");
		});

		assertEquals(originalBytes, Files.readString(file, StandardCharsets.UTF_8));
		assertTrue(Files.exists(Path.of(file + AetherConfig.BACKUP_SUFFIX)));
		try (var files = Files.list(directory)) {
			assertFalse(files.anyMatch(path -> path.getFileName().toString().endsWith(".tmp")),
					"staged files must be cleaned after a failed promotion");
		}
	}

	@Test
	@DisplayName("a corrupt primary recovers from the last known good backup without deleting either")
	void corruptPrimaryRecoversFromBackup(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		AetherConfig initial = new AetherConfig();
		initial.accentColor = "#2ECC8F";
		initial.save(file);

		AetherConfig replacement = new AetherConfig();
		replacement.accentColor = "#D99A69";
		replacement.save(file);

		Path backup = Path.of(file + AetherConfig.BACKUP_SUFFIX);
		Files.writeString(file, "{ definitely not json", StandardCharsets.UTF_8);

		AetherConfig recovered = AetherConfig.load(file);

		assertEquals("#2ECC8F", recovered.accentColor);
		assertEquals(AetherConfig.LoadStatus.RESTORED_FROM_BACKUP, recovered.getLoadStatus());
		assertEquals("{ definitely not json", Files.readString(file, StandardCharsets.UTF_8));
		assertTrue(Files.exists(backup));
		assertNotNull(recovered.getRecoveryMessage());
	}

	@Test
	@DisplayName("when both files are corrupt, defaults are used and originals are preserved")
	void corruptPrimaryAndBackupArePreserved(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		Path backup = Path.of(file + AetherConfig.BACKUP_SUFFIX);
		Files.writeString(file, "broken-primary", StandardCharsets.UTF_8);
		Files.writeString(backup, "broken-backup", StandardCharsets.UTF_8);

		AetherConfig recovered = AetherConfig.load(file);

		assertEquals(AetherConfig.LoadStatus.DEFAULTS_AFTER_CORRUPTION, recovered.getLoadStatus());
		assertEquals("broken-primary", Files.readString(file, StandardCharsets.UTF_8));
		assertEquals("broken-backup", Files.readString(backup, StandardCharsets.UTF_8));
		assertNotNull(recovered.getRecoveryMessage());
	}

	@Test
	@DisplayName("an unversioned legacy file is migrated and backed up")
	void legacyConfigurationIsVersioned(@TempDir Path directory) throws Exception {
		Path file = directory.resolve("config.json");
		String legacy = "{\\"accentColor\\":\\"#2ECC8F\\",\\"modules\\":{\\"fps\\":true}}";
		Files.writeString(file, legacy, StandardCharsets.UTF_8);

		AetherConfig loaded = AetherConfig.load(file);

		assertEquals(AetherConfig.CURRENT_SCHEMA_VERSION, loaded.schemaVersion);
		assertEquals(AetherConfig.LoadStatus.MIGRATED, loaded.getLoadStatus());
		assertEquals("#2ECC8F", loaded.accentColor);
		assertTrue(Files.readString(file).contains("\\"schemaVersion\\": " + AetherConfig.CURRENT_SCHEMA_VERSION));
		assertEquals(legacy, Files.readString(Path.of(file + AetherConfig.BACKUP_SUFFIX)));
	}

	@Test
	@DisplayName("concurrent saves to separate files use independent unique temporary files")
	void concurrentSavesToSeparateFiles(@TempDir Path directory) throws Exception {
		Path first = directory.resolve("first.json");
		Path second = directory.resolve("second.json");
		AetherConfig firstConfig = new AetherConfig();
		firstConfig.accentColor = "#2ECC8F";
		AetherConfig secondConfig = new AetherConfig();
		secondConfig.accentColor = "#D99A69";

		CompletableFuture<Void> firstSave = CompletableFuture.runAsync(() -> firstConfig.save(first));
		CompletableFuture<Void> secondSave = CompletableFuture.runAsync(() -> secondConfig.save(second));
		CompletableFuture.allOf(firstSave, secondSave).get(10, TimeUnit.SECONDS);

		assertEquals("#2ECC8F", AetherConfig.load(first).accentColor);
		assertEquals("#D99A69", AetherConfig.load(second).accentColor);
	}
}

package dev.aether.client.config;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.locks.ReentrantLock;

/**
 * Detects consecutive sessions that did not reach Fabric's normal client-stopping callback.
 *
 * <p>This is evidence of an unclean shutdown only. It is not proof that Aether caused a crash.
 * The caller may offer the player a one-session safe mode after repeated abnormal shutdowns.</p>
 */
public final class ClientRecoveryGuard {
	private static final int MAX_RECORDED_STREAK = 99;
	private static final ConcurrentMap<Path, ReentrantLock> LOCKS = new ConcurrentHashMap<>();

	public record Startup(int uncleanStartStreak, boolean recoveryRecommended) {
	}

	private ClientRecoveryGuard() {
	}

	/**
	 * Records that a new session has started. The marker is intentionally kept until
	 * {@link #finish(Path)} is called from the normal client-stopping lifecycle event.
	 */
	public static Startup begin(Path marker) throws IOException {
		Path target = marker.toAbsolutePath().normalize();
		ReentrantLock lock = LOCKS.computeIfAbsent(target, ignored -> new ReentrantLock());
		lock.lock();
		try {
			Files.createDirectories(target.getParent());
			int previous = readPreviousStreak(target);
			int current = previous < 0 ? 0 : Math.min(previous + 1, MAX_RECORDED_STREAK);
			writeAtomically(target, Integer.toString(current) + System.lineSeparator());
			return new Startup(current, current >= 2);
		} finally {
			lock.unlock();
		}
	}

	/**
	 * Clears the marker after an orderly client shutdown.
	 */
	public static void finish(Path marker) throws IOException {
		Path target = marker.toAbsolutePath().normalize();
		ReentrantLock lock = LOCKS.computeIfAbsent(target, ignored -> new ReentrantLock());
		lock.lock();
		try {
			Files.deleteIfExists(target);
		} finally {
			lock.unlock();
		}
	}

	private static int readPreviousStreak(Path marker) throws IOException {
		if (!Files.exists(marker)) {
			return -1;
		}
		try {
			int value = Integer.parseInt(Files.readString(marker, StandardCharsets.UTF_8).trim());
			if (value < 0 || value > MAX_RECORDED_STREAK) {
				throw new NumberFormatException("streak outside supported range");
			}
			return value;
		} catch (NumberFormatException exception) {
			Path preserved = Files.createTempFile(marker.getParent(), "session.guard.corrupt-", ".txt");
			try {
				Files.copy(marker, preserved, StandardCopyOption.REPLACE_EXISTING);
				AetherClientLogger.warn("An invalid Aether session marker was preserved at " + preserved
						+ ". Crash attribution is unavailable.");
			} catch (IOException copyFailure) {
				Files.deleteIfExists(preserved);
				throw copyFailure;
			}
			return -1;
		}
	}

	private static void writeAtomically(Path destination, String value) throws IOException {
		Path staged = Files.createTempFile(destination.getParent(), "session.guard.", ".tmp");
		boolean promoted = false;
		try {
			Files.writeString(staged, value, StandardCharsets.UTF_8);
			try {
				Files.move(staged, destination, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
			} catch (AtomicMoveNotSupportedException | FileAlreadyExistsException exception) {
				Files.move(staged, destination, StandardCopyOption.REPLACE_EXISTING);
			}
			promoted = true;
		} finally {
			if (!promoted) {
				Files.deleteIfExists(staged);
			}
		}
	}
}

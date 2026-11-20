package dev.aether.client.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.setting.Setting;
import dev.aether.client.util.ColorUtil;
import net.fabricmc.loader.api.FabricLoader;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.FileAlreadyExistsException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentMap;
import java.util.concurrent.locks.ReentrantLock;

/**
 * The persistent configuration of the client.
 *
 * <p>Configuration writes are staged in unique sibling files and atomically promoted. Before a
 * valid primary file is replaced, its bytes are atomically copied to a last-known-good backup.</p>
 */
public final class AetherConfig {
	public static final String DEFAULT_ACCENT = "#E7E9EC";
	public static final int CURRENT_SCHEMA_VERSION = 1;
	public static final String BACKUP_SUFFIX = ".bak";

	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
	private static final Set<String> LEGACY_ACCENTS =
			Set.of("#6C5CE7", "#7C5CFC", "#3B82F6");
	private static final ConcurrentMap<Path, ReentrantLock> SAVE_LOCKS = new ConcurrentHashMap<>();

	public enum LoadStatus {
		MISSING,
		LOADED,
		MIGRATED,
		RESTORED_FROM_BACKUP,
		DEFAULTS_AFTER_CORRUPTION
	}

	@FunctionalInterface
	interface AtomicMove {
		void move(Path source, Path destination) throws IOException;
	}

	/** Version of the persisted JSON schema. Files without this field are version 0. */
	public int schemaVersion = CURRENT_SCHEMA_VERSION;

	/** The accent colour used by the interface. */
	public String accentColor = DEFAULT_ACCENT;

	/** Whether the client shows a notification when it joins or leaves a world. */
	public boolean showNotifications = true;

	/** Whether the click GUI opens with the right shift key. */
	public boolean openClickGuiWithRightShift = true;

	/** The enabled state of every module, keyed by the lower case module name. */
	public Map<String, Boolean> modules = new LinkedHashMap<>();

	/** The settings of every module, keyed by the lower case module name. */
	public Map<String, Map<String, Object>> settings = new LinkedHashMap<>();

	/** The position of every HUD element, keyed by the lower case element name. */
	public Map<String, int[]> hudPositions = new LinkedHashMap<>();

	private transient LoadStatus loadStatus = LoadStatus.MISSING;

	public static Path getPath() {
		return FabricLoader.getInstance().getConfigDir().resolve("aether").resolve("config.json");
	}

	public static AetherConfig load() {
		return load(getPath());
	}

	/**
	 * Loads the primary configuration, then the last-known-good backup if the primary is missing or
	 * unreadable. Corrupt files are never deleted or overwritten during recovery.
	 */
	public static AetherConfig load(Path path) {
		Path primary = path.toAbsolutePath().normalize();
		boolean primaryExists = Files.exists(primary);
		IOException primaryFailure = null;

		if (primaryExists) {
			try {
				AetherConfig config = readConfiguration(primary);
				if (config.loadStatus == LoadStatus.MIGRATED) {
					AetherClientLogger.info("Migrating the Aether configuration to schema "
							+ CURRENT_SCHEMA_VERSION + ".");
					config.save(primary);
				}
				return config;
			} catch (IOException | RuntimeException exception) {
				primaryFailure = asIOException("The primary configuration is unreadable.", exception);
				AetherClientLogger.warn(primaryFailure.getMessage());
			}
		}

		Path backup = backupPath(primary);
		if (Files.exists(backup)) {
			try {
				AetherConfig recovered = readConfiguration(backup);
				recovered.loadStatus = LoadStatus.RESTORED_FROM_BACKUP;
				AetherClientLogger.warn("Aether loaded the last-known-good configuration from "
						+ backup + ". The original file was preserved.");
				return recovered;
			} catch (IOException | RuntimeException exception) {
				AetherClientLogger.error("The Aether configuration backup is also unreadable.", exception);
			}
		}

		AetherConfig defaults = new AetherConfig();
		defaults.sanitize();
		defaults.loadStatus = primaryExists || Files.exists(backup)
				? LoadStatus.DEFAULTS_AFTER_CORRUPTION
				: LoadStatus.MISSING;
		if (primaryFailure != null) {
			AetherClientLogger.error("Aether is using defaults. The primary file and backup were left untouched.",
					primaryFailure);
		}
		return defaults;
	}

	private static AetherConfig readConfiguration(Path path) throws IOException {
		String json = stripByteOrderMark(Files.readString(path, StandardCharsets.UTF_8));
		JsonElement parsed = JsonParser.parseString(json);
		if (parsed == null || !parsed.isJsonObject()) {
			throw new IOException("The configuration root must be a JSON object: " + path);
		}

		JsonObject root = parsed.getAsJsonObject();
		int storedVersion = 0;
		if (root.has("schemaVersion")) {
			JsonElement version = root.get("schemaVersion");
			if (version == null || !version.isJsonPrimitive() || !version.getAsJsonPrimitive().isNumber()) {
				throw new IOException("The configuration schemaVersion is not a number: " + path);
			}
			try {
				storedVersion = version.getAsInt();
			} catch (RuntimeException exception) {
				throw asIOException("The configuration schemaVersion is invalid: " + path, exception);
			}
		}
		if (storedVersion < 0 || storedVersion > CURRENT_SCHEMA_VERSION) {
			throw new IOException("Unsupported Aether configuration schema " + storedVersion + ": " + path);
		}

		AetherConfig config = GSON.fromJson(root, AetherConfig.class);
		if (config == null) {
			throw new IOException("The configuration is empty: " + path);
		}
		config.schemaVersion = storedVersion;
		config.migrate();
		config.sanitize();
		config.loadStatus = storedVersion == CURRENT_SCHEMA_VERSION ? LoadStatus.LOADED : LoadStatus.MIGRATED;
		return config;
	}

	private boolean migrate() {
		if (schemaVersion >= CURRENT_SCHEMA_VERSION) {
			return false;
		}
		// Version 0 is the original unversioned Aether JSON layout. Its fields are unchanged;
		// assigning version 1 makes subsequent migrations explicit and testable.
		schemaVersion = CURRENT_SCHEMA_VERSION;
		return true;
	}

	private static String stripByteOrderMark(String json) {
		if (!json.isEmpty() && json.charAt(0) == '\uFEFF') {
			return json.substring(1);
		}
		return json;
	}

	private void sanitize() {
		if (accentColor == null || accentColor.isBlank()
				|| LEGACY_ACCENTS.contains(accentColor.trim().toUpperCase(java.util.Locale.ROOT))) {
			accentColor = DEFAULT_ACCENT;
		} else {
			accentColor = accentColor.trim();
		}
		if (modules == null) {
			modules = new LinkedHashMap<>();
		}
		if (settings == null) {
			settings = new LinkedHashMap<>();
		}
		if (hudPositions == null) {
			hudPositions = new LinkedHashMap<>();
		}
	}

	public LoadStatus getLoadStatus() {
		return loadStatus;
	}

	/**
	 * A player-facing explanation for a non-destructive recovery. A null value means no notice is
	 * necessary.
	 */
	public String getRecoveryMessage() {
		return switch (loadStatus) {
			case RESTORED_FROM_BACKUP ->
					"Aether restored your last valid configuration from its backup. The damaged file was preserved.";
			case DEFAULTS_AFTER_CORRUPTION ->
					"Aether could not read its configuration or backup. Safe defaults are active; original files were preserved.";
			default -> null;
		};
	}

	public void save() {
		save(getPath());
	}

	public void save(Path path) {
		save(path, AetherConfig::moveReplacing);
	}

	/**
	 * Package-private commit hook for deterministic failure tests. Production callers use the
	 * regular atomic move implementation.
	 */
	void save(Path path, AtomicMove mainCommit) {
		Path target = path.toAbsolutePath().normalize();
		ReentrantLock lock = SAVE_LOCKS.computeIfAbsent(target, ignored -> new ReentrantLock());
		lock.lock();
		Path staged = null;
		try {
			Path parent = target.getParent();
			Files.createDirectories(parent);
			schemaVersion = CURRENT_SCHEMA_VERSION;
			byte[] bytes = GSON.toJson(this).getBytes(StandardCharsets.UTF_8);
			staged = writeTemporary(target, bytes);

			if (Files.exists(target)) {
				if (isValidConfiguration(target)) {
					writeAtomically(backupPath(target), Files.readAllBytes(target));
				} else {
					preserveCorruptFile(target);
				}
			}

			mainCommit.move(staged, target);
			staged = null;
		} catch (IOException | RuntimeException exception) {
			AetherClientLogger.warn("Could not safely write the Aether configuration: " + exception.getMessage());
		} finally {
			if (staged != null) {
				try {
					Files.deleteIfExists(staged);
				} catch (IOException exception) {
					AetherClientLogger.warn("Could not remove a staged configuration file: " + exception.getMessage());
				}
			}
			lock.unlock();
		}
	}

	private static boolean isValidConfiguration(Path path) {
		try {
			readConfiguration(path);
			return true;
		} catch (IOException | RuntimeException exception) {
			return false;
		}
	}

	private static Path backupPath(Path primary) {
		return primary.resolveSibling(primary.getFileName().toString() + BACKUP_SUFFIX);
	}

	private static void preserveCorruptFile(Path source) throws IOException {
		Path parent = source.getParent();
		String prefix = source.getFileName() + ".corrupt-";
		if (prefix.length() < 3) {
			prefix = "cfg-corrupt-";
		}
		Path preserved = Files.createTempFile(parent, prefix, ".json");
		try {
			Files.copy(source, preserved, StandardCopyOption.REPLACE_EXISTING);
			AetherClientLogger.warn("Preserved an unreadable configuration at " + preserved + ".");
		} catch (IOException exception) {
			Files.deleteIfExists(preserved);
			throw exception;
		}
	}

	private static void writeAtomically(Path destination, byte[] bytes) throws IOException {
		Path staged = writeTemporary(destination, bytes);
		try {
			moveReplacing(staged, destination);
			staged = null;
		} finally {
			if (staged != null) {
				Files.deleteIfExists(staged);
			}
		}
	}

	private static Path writeTemporary(Path destination, byte[] bytes) throws IOException {
		Path parent = destination.getParent();
		String filename = destination.getFileName().toString();
		String prefix = filename.length() >= 3 ? filename + "." : "cfg-" + filename + ".";
		Path staged = Files.createTempFile(parent, prefix, ".tmp");
		boolean complete = false;
		try (FileChannel channel = FileChannel.open(staged,
				StandardOpenOption.WRITE, StandardOpenOption.TRUNCATE_EXISTING)) {
			ByteBuffer buffer = ByteBuffer.wrap(bytes);
			while (buffer.hasRemaining()) {
				channel.write(buffer);
			}
			channel.force(true);
			complete = true;
			return staged;
		} finally {
			if (!complete) {
				Files.deleteIfExists(staged);
			}
		}
	}

	private static void moveReplacing(Path source, Path destination) throws IOException {
		try {
			Files.move(source, destination, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
		} catch (AtomicMoveNotSupportedException | FileAlreadyExistsException exception) {
			Files.move(source, destination, StandardCopyOption.REPLACE_EXISTING);
		}
	}

	private static IOException asIOException(String message, Exception cause) {
		return cause instanceof IOException io ? io : new IOException(message, cause);
	}

	public void applyTo(ModuleManager manager) {
		applyTo(manager, true);
	}

	/**
	 * Applies persisted settings and optionally restores the persisted enabled flags. Safe recovery
	 * passes false so the player's settings are loaded without reactivating modules for this session.
	 */
	public void applyTo(ModuleManager manager, boolean restoreEnabledStates) {
		for (Module module : manager.getAll()) {
			String key = module.getName().toLowerCase(java.util.Locale.ROOT);
			Map<String, Object> stored = settings.get(key);
			if (stored != null) {
				for (Setting setting : module.getSettings()) {
					Object value = stored.get(setting.getName());
					if (value != null) {
						setting.setValue(value);
					}
				}
			}
			if (restoreEnabledStates) {
				Boolean enabled = modules.get(key);
				if (enabled != null) {
					module.setEnabled(enabled);
				}
			} else {
				module.setEnabled(false);
			}
		}
	}

	public void captureFrom(ModuleManager manager) {
		modules.clear();
		settings.clear();
		for (Module module : manager.getAll()) {
			String key = module.getName().toLowerCase(java.util.Locale.ROOT);
			modules.put(key, module.isEnabled());
			Map<String, Object> moduleSettings = new LinkedHashMap<>();
			for (Setting setting : module.getSettings()) {
				moduleSettings.put(setting.getName(), setting.getValue());
			}
			if (!moduleSettings.isEmpty()) {
				settings.put(key, moduleSettings);
			}
		}
	}

	public int getAccentColor() {
		return ColorUtil.fromHex(accentColor, ColorUtil.fromHex(DEFAULT_ACCENT, 0xFF6C5CE7));
	}

	public void resetSettings(ModuleManager manager) {
		for (Module module : manager.getAll()) {
			for (Setting setting : module.getSettings()) {
				setting.reset();
			}
		}
	}
}

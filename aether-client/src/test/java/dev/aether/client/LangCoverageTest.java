package dev.aether.client;

import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Checks the language file of the client.
 *
 * <p>The game prints the raw translation key when a key is missing: the controls screen showed
 * {@code key.aether.click_gui} and {@code key.aether.kill_aura} because the mod shipped no
 * {@code en_us.json} at all. This test keeps the file complete: every module that creates a keybind,
 * and the two keys of the client, must have a label.</p>
 */
class LangCoverageTest {

	private static final Path SOURCES = Path.of("src/main/java/dev/aether/client");
	private static final Path LANG = Path.of("src/main/resources/assets/aether/lang/en_us.json");

	@Test
	@DisplayName("every keybind of the client has a label")
	void everyKeybindIsTranslated() throws IOException {
		String lang = Files.readString(LANG, StandardCharsets.UTF_8);
		List<String> missing = new ArrayList<>();

		for (String key : requestedKeys()) {
			if (!lang.contains("\"" + key + "\"")) {
				missing.add(key);
			}
		}

		assertTrue(missing.isEmpty(), "missing translations: " + missing);
	}

	@Test
	@DisplayName("the labels are not the keys themselves")
	void labelsAreRealLabels() throws IOException {
		String lang = Files.readString(LANG, StandardCharsets.UTF_8);
		assertTrue(lang.contains("\"key.aether.click_gui\": \"Click GUI\""), "the click GUI key has no label");
		assertTrue(lang.contains("\"key.aether.hud_editor\": \"Aether configuration\""),
				"the configuration key has no label");
	}

	/**
	 * @return every {@code key.aether.…} the client can display: the two fixed keys, plus the key of
	 *         each module, built from its name the same way {@code Module.createKeyBinding} does.
	 */
	private static List<String> requestedKeys() throws IOException {
		List<String> keys = new ArrayList<>(List.of("key.aether.click_gui", "key.aether.hud_editor"));
		Pattern constructor = Pattern.compile("super\\(\"([^\"]+)\"");
		Pattern keybind = Pattern.compile("new KeyMapping\\(\\s*\"(key\\.aether\\.[a-z_]+)\"");

		try (Stream<Path> files = Files.walk(SOURCES)) {
			for (Path file : files.filter(path -> path.toString().endsWith(".java")).toList()) {
				String text = Files.readString(file, StandardCharsets.UTF_8);
				Matcher module = constructor.matcher(text);
				while (module.find()) {
					keys.add("key.aether." + module.group(1).toLowerCase(Locale.ROOT).replace(' ', '_'));
				}
				Matcher direct = keybind.matcher(text);
				while (direct.find()) {
					keys.add(direct.group(1));
				}
			}
		}
		return keys.stream().distinct().toList();
	}
}

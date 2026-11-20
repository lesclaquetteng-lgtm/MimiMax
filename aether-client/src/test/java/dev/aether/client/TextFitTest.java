package dev.aether.client;

import dev.aether.client.util.TextFit;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

import java.util.Random;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.ToIntFunction;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Tests the width fitting of the texts, and counts how often it asks for a measurement.
 */
class TextFitTest {
	/** The width of one character, in pixels, used by these tests. */
	private static final int CHARACTER_WIDTH = 6;

	/** The width the game would return for a string, without a client. */
	private static ToIntFunction<String> width() {
		return value -> value.codePointCount(0, value.length()) * CHARACTER_WIDTH;
	}

	/** The character-by-character version this replaces, kept here as the reference. */
	private static String truncateOneByOne(String value, int maxWidth, ToIntFunction<String> measure) {
		if (measure.applyAsInt(value) <= maxWidth) {
			return value;
		}
		String trimmed = value;
		while (trimmed.length() > 1 && measure.applyAsInt(trimmed + TextFit.ELLIPSIS) > maxWidth) {
			trimmed = trimmed.substring(0, trimmed.length() - 1);
		}
		return trimmed + TextFit.ELLIPSIS;
	}

	private static String text(int length) {
		StringBuilder builder = new StringBuilder(length);
		for (int i = 0; i < length; i++) {
			builder.append((char) ('a' + (i % 26)));
		}
		return builder.toString();
	}

	private static int measuredCalls(String value, int maxWidth, AtomicInteger counter) {
		ToIntFunction<String> counting = text -> {
			counter.incrementAndGet();
			return width().applyAsInt(text);
		};
		TextFit.truncate(value, maxWidth, counting);
		return counter.get();
	}

	@Test
	@DisplayName("a text that fits is returned untouched, with a single measurement")
	void fitsUntouched() {
		AtomicInteger calls = new AtomicInteger();
		String value = "Watermark";

		assertEquals(value, TextFit.truncate(value, 200, width()));
		assertEquals(1, measuredCalls(value, 200, calls), "a fitting text must be measured once");
	}

	@Test
	@DisplayName("the longest fitting prefix is kept")
	void longestPrefix() {
		// Nine characters plus the ellipsis is ten: 60 pixels, the width available.
		assertEquals("abcdefghi" + TextFit.ELLIPSIS, TextFit.truncate(text(30), 60, width()));
		assertEquals("abc" + TextFit.ELLIPSIS, TextFit.truncate("abcdefgh", 24, width()));
	}

	@Test
	@DisplayName("at least one character is kept when nothing fits")
	void keepsOneCharacter() {
		assertEquals("a" + TextFit.ELLIPSIS, TextFit.truncate("abcdef", 0, width()));
		// An empty text already fits, whatever the width.
		assertEquals("", TextFit.truncate("", 0, width()));
	}

	@Test
	@DisplayName("the dichotomy picks the same prefix as the character by character version")
	void sameResultAsTheReference() {
		Random random = new Random(20261115L);

		for (int round = 0; round < 500; round++) {
			String value = text(1 + random.nextInt(240));
			int maxWidth = random.nextInt(600);

			assertEquals(
				truncateOneByOne(value, maxWidth, width()),
				TextFit.truncate(value, maxWidth, width()),
				"different result for " + value.length() + " characters and a width of " + maxWidth);
		}
	}

	@Test
	@DisplayName("the dichotomy measures far less often than the character by character version")
	void measuresLessOften() {
		String value = text(300);
		int maxWidth = 120;

		AtomicInteger oneByOne = new AtomicInteger();
		truncateOneByOne(value, maxWidth, text -> {
			oneByOne.incrementAndGet();
			return width().applyAsInt(text);
		});

		AtomicInteger dichotomy = new AtomicInteger();
		measuredCalls(value, maxWidth, dichotomy);

		// 300 characters need 9 halvings, plus the final check of the whole string.
		assertTrue(dichotomy.get() <= 12, "the dichotomy measured " + dichotomy.get() + " times");
		assertTrue(oneByOne.get() > 200, "the reference measured only " + oneByOne.get() + " times");
		assertTrue(
			oneByOne.get() > dichotomy.get() * 15,
			"expected a large gain, got " + oneByOne.get() + " against " + dichotomy.get());
	}
}

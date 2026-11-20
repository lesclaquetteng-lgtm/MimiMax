package dev.aether.client.config;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * A tiny logging helper so that the configuration code does not have to import the logger
 * everywhere.
 */
public final class AetherClientLogger {
	private static final Logger LOGGER = LoggerFactory.getLogger("Aether Client");

	private AetherClientLogger() {
	}

	public static void info(String message) {
		LOGGER.info(message);
	}

	public static void warn(String message) {
		LOGGER.warn(message);
	}

	public static void error(String message, Throwable throwable) {
		LOGGER.error(message, throwable);
	}
}

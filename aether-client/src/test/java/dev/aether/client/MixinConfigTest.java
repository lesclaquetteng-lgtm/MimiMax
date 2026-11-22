package dev.aether.client;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;
import org.objectweb.asm.AnnotationVisitor;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.Type;
import org.spongepowered.asm.mixin.injection.Inject;

import java.io.InputStream;
import java.io.InputStreamReader;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Makes sure that the mixin configuration and the mixin classes always match and that every
 * injection point still exists in the game.
 *
 * A mixin that is listed but does not exist, or that points at a method that was renamed, only
 * fails when the game starts. This test catches both cases long before.
 */
class MixinConfigTest {

	private static List<String> readMixinNames() throws Exception {
		try (InputStream stream = MixinConfigTest.class.getResourceAsStream("/aether.mixins.json")) {
			assertNotNull(stream, "the mixin configuration is missing");

			JsonObject configuration = JsonParser
					.parseReader(new InputStreamReader(stream, StandardCharsets.UTF_8))
					.getAsJsonObject();

			String packageName = configuration.get("package").getAsString();
			JsonArray mixins = configuration.getAsJsonArray("client");

			List<String> names = new ArrayList<>();
			for (int i = 0; i < mixins.size(); i++) {
				names.add(packageName + "." + mixins.get(i).getAsString());
			}
			return names;
		}
	}

	/**
	 * Reads the class a mixin targets.
	 *
	 * The {@code @Mixin} annotation is not kept at runtime, the class file has to be read to find
	 * out which class is patched.
	 *
	 * @param mixin the mixin to inspect
	 * @return the name of the patched class
	 */
	private static String readTargetOf(Class<?> mixin) throws Exception {
		String[] result = new String[1];
		String resource = mixin.getName().replace('.', '/') + ".class";

		try (InputStream stream = mixin.getClassLoader().getResourceAsStream(resource)) {
			assertNotNull(stream, "the class file of " + mixin.getName() + " is missing");

			ClassReader reader = new ClassReader(stream);
			reader.accept(new ClassVisitor(Opcodes.ASM9) {
				@Override
				public AnnotationVisitor visitAnnotation(String descriptor, boolean visible) {
					if (!descriptor.equals("Lorg/spongepowered/asm/mixin/Mixin;")) {
						return null;
					}
					return new AnnotationVisitor(Opcodes.ASM9) {
						@Override
						public void visit(String name, Object value) {
							if ("value".equals(name) && value instanceof Type type) {
								result[0] = type.getClassName();
							}
						}

						@Override
						public AnnotationVisitor visitArray(String name) {
							if (!"value".equals(name)) {
								return null;
							}
							// The targets of a mixin are an array, every entry is a class.
							return new AnnotationVisitor(Opcodes.ASM9) {
								@Override
								public void visit(String ignored, Object value) {
									if (value instanceof Type type) {
										result[0] = type.getClassName();
									}
								}
							};
						}
					};
				}
			}, ClassReader.SKIP_CODE);
		}

		return result[0];
	}

	@Test
	@DisplayName("every mixin of the configuration exists")
	void everyMixinExists() throws Exception {
		List<String> names = readMixinNames();
		assertTrue(names.size() > 0, "the configuration does not contain any mixin");

		for (String name : names) {
			Class<?> mixin = Class.forName(name);
			assertTrue(mixin.getName().startsWith("dev.aether.client.mixin"),
					"the mixin " + name + " is in the wrong package");
		}
	}

	@Test
	@DisplayName("every mixin patches Minecraft or its bundled Realms client")
	void everyMixinPatchesAGameClass() throws Exception {
		for (String name : readMixinNames()) {
			Class<?> mixin = Class.forName(name);
			String target = readTargetOf(mixin);

			assertNotNull(target, name + " does not declare a target class");
			boolean isMinecraftTarget = target.startsWith("net.minecraft.");
			boolean isBundledRealmsTarget = target.startsWith("com.mojang.realmsclient.");
			assertTrue(isMinecraftTarget || isBundledRealmsTarget,
					name + " patches " + target + " which is not part of the Minecraft client");
		}
	}

	@Test
	@DisplayName("every injection point still exists in the game")
	void everyInjectionPointExists() throws Exception {
		for (String name : readMixinNames()) {
			Class<?> mixin = Class.forName(name);
			String targetName = readTargetOf(mixin);
			assertNotNull(targetName, name + " does not declare a target class");

			// Loaded without initializing: reading the methods of a game class is enough, and the
			// static initializer of an entity class registers particles and fails outside a
			// bootstrapped game.
			Class<?> target = Class.forName(targetName, false, MixinConfigTest.class.getClassLoader());

			for (Method method : mixin.getDeclaredMethods()) {
				Inject inject = method.getAnnotation(Inject.class);
				if (inject == null) {
					continue;
				}

				for (String rawTarget : inject.method()) {
					boolean hasDescriptor = rawTarget.contains("(");
					String simpleName = hasDescriptor
							? rawTarget.substring(0, rawTarget.indexOf('('))
							: rawTarget;

					// The parameters narrow the search to the overload the mixin really names. A
					// class may declare two methods under one name, and returning the first one
					// would compare the descriptor of the wrong overload.
					Class<?>[] declared = hasDescriptor ? parametersOf(rawTarget) : new Class<?>[0];
					Method targetMethod = findMethod(target, simpleName, declared);
					assertNotNull(targetMethod, String.format(
							"%s injects into %s#%s but that method does not exist",
							name, target.getName(), simpleName));

					// A name is not enough: an overload with another signature would let the test pass
					// while the mixin fails when the game starts.
					if (hasDescriptor) {
						assertArrayEquals(declared, targetMethod.getParameterTypes(), String.format(
								"%s injects into %s#%s but the parameters of the game method differ",
								name, target.getName(), rawTarget));
					}

					// A method that returns a value needs a CallbackInfoReturnable, otherwise the
					// mixin fails while the game starts.
					boolean returnsValue = targetMethod.getReturnType() != void.class;
					boolean usesReturnable = method.getParameterTypes().length > 0
							&& method.getParameterTypes()[method.getParameterTypes().length - 1]
							.getName().endsWith("CallbackInfoReturnable");

					assertEquals(returnsValue, usesReturnable, String.format(
							"%s injects into %s#%s, the callback does not match the return type",
							name, target.getName(), simpleName));
				}
			}
		}
	}

	/**
	 * Searches a method in a class and in every class it inherits from.
	 *
	 * <p>When the mixin names a descriptor, the overload with exactly those parameters wins; the
	 * first one found under the name is only a fallback, so that a real signature mismatch is
	 * reported as a mismatch instead of the test claiming the method does not exist.</p>
	 *
	 * @param type       the class to search in
	 * @param method     the name of the method
	 * @param parameters the parameters of the overload, empty when the mixin names none
	 * @return the method or null
	 */
	private static Method findMethod(Class<?> type, String method, Class<?>... parameters) {
		Method fallback = null;
		Class<?> current = type;
		while (current != null) {
			for (Method candidate : current.getDeclaredMethods()) {
				if (!candidate.getName().equals(method)) {
					continue;
				}
				if (Arrays.equals(candidate.getParameterTypes(), parameters)) {
					return candidate;
				}
				if (fallback == null) {
					fallback = candidate;
				}
			}
			current = current.getSuperclass();
		}
		return fallback;
	}

	/**
	 * Reads the parameter types of a descriptor such as {@code knockback(DDD)V}.
	 *
	 * @param descriptor the method descriptor written in the mixin
	 * @return the parameter types, in order
	 */
	private static Class<?>[] parametersOf(String descriptor) throws ClassNotFoundException {
		String inside = descriptor.substring(descriptor.indexOf('(') + 1, descriptor.indexOf(')'));
		List<Class<?>> types = new ArrayList<>();
		int index = 0;
		while (index < inside.length()) {
			char code = inside.charAt(index++);
			switch (code) {
				case 'D' -> types.add(double.class);
				case 'F' -> types.add(float.class);
				case 'I' -> types.add(int.class);
				case 'J' -> types.add(long.class);
				case 'Z' -> types.add(boolean.class);
				case 'S' -> types.add(short.class);
				case 'B' -> types.add(byte.class);
				case 'C' -> types.add(char.class);
				case 'L' -> {
					int end = inside.indexOf(';', index);
					types.add(Class.forName(inside.substring(index, end).replace('/', '.'), false,
							MixinConfigTest.class.getClassLoader()));
					index = end + 1;
				}
				default -> throw new IllegalArgumentException("unsupported descriptor: " + descriptor);
			}
		}
		return types.toArray(new Class<?>[0]);
	}

	@Test
	@DisplayName("every mixin injects into the game")
	void everyMixinInjects() throws Exception {
		for (String name : readMixinNames()) {
			Class<?> mixin = Class.forName(name);

			int injections = 0;
			for (Method method : mixin.getDeclaredMethods()) {
				if (method.getAnnotation(Inject.class) != null) {
					injections++;
				}
			}
			assertTrue(injections > 0, name + " does not contain a single injection");
		}
	}

	@Test
	@DisplayName("the mixin configuration is valid")
	void configurationIsValid() throws Exception {
		try (InputStream stream = getClass().getResourceAsStream("/aether.mixins.json")) {
			assertNotNull(stream);
			JsonObject configuration = JsonParser
					.parseReader(new InputStreamReader(stream, StandardCharsets.UTF_8))
					.getAsJsonObject();

			assertEquals("dev.aether.client.mixin", configuration.get("package").getAsString());
			assertEquals("JAVA_21", configuration.get("compatibilityLevel").getAsString());
			assertTrue(configuration.get("required").getAsBoolean(),
					"the mixins must be required so that a broken mixin stops the game");
		}
	}

	@Test
	@DisplayName("the mod metadata is valid")
	void modMetadataIsValid() throws Exception {
		try (InputStream stream = getClass().getResourceAsStream("/fabric.mod.json")) {
			assertNotNull(stream, "the mod metadata is missing");

			JsonObject metadata = JsonParser
					.parseReader(new InputStreamReader(stream, StandardCharsets.UTF_8))
					.getAsJsonObject();

			assertEquals("aether", metadata.get("id").getAsString());
			assertEquals("client", metadata.get("environment").getAsString());
			assertTrue(metadata.has("entrypoints"), "the mod has no entry point");

			JsonArray entrypoints = metadata.getAsJsonObject("entrypoints").getAsJsonArray("client");
			assertEquals(1, entrypoints.size());
			assertEquals("dev.aether.client.AetherClient", entrypoints.get(0).getAsString());
			Class.forName(entrypoints.get(0).getAsString());

			JsonArray mixins = metadata.getAsJsonArray("mixins");
			assertEquals(1, mixins.size());
			assertEquals("aether.mixins.json", mixins.get(0).getAsString());
		}
	}
}

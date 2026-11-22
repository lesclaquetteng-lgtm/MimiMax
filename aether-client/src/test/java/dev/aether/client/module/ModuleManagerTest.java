package dev.aether.client.module;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotSame;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;

class ModuleManagerTest {
	@Test
	void the_ordered_module_view_is_cached_immutable_and_updated_on_registration() {
		ModuleManager manager = new ModuleManager();
		List<Module> empty = manager.getAll();

		assertSame(empty, manager.getAll(), "reading modules should reuse the cached view");
		assertThrows(UnsupportedOperationException.class, () -> empty.add(module("First")));

		Module first = module("First");
		manager.register(first);
		List<Module> afterFirst = manager.getAll();
		assertNotSame(empty, afterFirst);
		assertEquals(List.of(first), afterFirst);
		assertSame(afterFirst, manager.getAll());

		Module second = module("Second");
		manager.register(second);
		assertEquals(List.of(first, second), manager.getAll());
		assertSame(afterFirst.get(0), manager.getAll().get(0));
	}

	private static Module module(String name) {
		return new Module(name, "test module", ModuleCategory.RENDER, false) {};
	}
}

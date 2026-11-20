package dev.aether.client;

import com.mojang.blaze3d.platform.InputConstants;
import dev.aether.client.config.AetherConfig;
import dev.aether.client.config.ClientRecoveryGuard;
import dev.aether.client.gui.ClickGuiScreen;
import dev.aether.client.gui.ConfigScreen;
import dev.aether.client.hud.HudManager;
import dev.aether.client.module.HudElement;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCatalog;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.module.ModuleManager;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.util.SessionUtil;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandManager;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.HudRenderCallback;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.ConfirmScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;

/**
 * The entry point of the Aether client.
 */
public class AetherClient implements ClientModInitializer {
	public static final String MOD_ID = "aether";
	public static final String NAME = "Aether Client";
	public static final String VERSION = "1.0.0";
	public static final Logger LOGGER = LoggerFactory.getLogger(NAME);

	private static final KeyMapping.Category CATEGORY =
			KeyMapping.Category.register(net.minecraft.resources.Identifier.fromNamespaceAndPath(MOD_ID, "general"));

	private static AetherClient instance;

	private final ModuleManager moduleManager = new ModuleManager();
	private final HudManager hudManager = new HudManager();

	/** The configuration of the last session, applied on the first tick (see {@link #onTick}). */
	private AetherConfig config;
	private boolean configApplied;
	private boolean recoveryNoticeShown;
	private boolean recoverySafeMode;
	private boolean recoveryDecisionMade;
	private boolean recoveryMarkerInitialized;
	private ClientRecoveryGuard.Startup recoveryStartup = new ClientRecoveryGuard.Startup(0, false);
	private Path recoveryMarker;

	private KeyMapping clickGuiKey;
	private KeyMapping hudEditorKey;

	@Override
	public void onInitializeClient() {
		instance = this;

		recoveryMarker = AetherConfig.getPath().resolveSibling("session.guard");
		try {
			recoveryStartup = ClientRecoveryGuard.begin(recoveryMarker);
			recoveryMarkerInitialized = true;
		} catch (IOException exception) {
			LOGGER.warn("Could not initialize the Aether session recovery marker; crash history is unavailable.",
					exception);
		}
		recoveryDecisionMade = !recoveryStartup.recoveryRecommended();
		config = AetherConfig.load();
		Aether aether = Aether.create(config, moduleManager);

		registerModules();

		registerKeyBindings();
		registerEvents();

		LOGGER.info("{} {} ready - session: {}", NAME, VERSION, SessionUtil.getSessionLabel());
	}

	/**
	 * Creates every module of the client.
	 *
	 * <p>The list lives in {@link ModuleCatalog} so that a test can build it and check that every key
	 * a mixin reads is registered, without starting the game.</p>
	 */
	private void registerModules() {
		for (Module module : ModuleCatalog.create()) {
			moduleManager.register(module);
			if (module instanceof HudElement element) {
				hudManager.register(element);
			}
		}
	}

	/**
	 * Creates the keybinds of the client.
	 */
	private void registerKeyBindings() {
		clickGuiKey = new KeyMapping(
				"key.aether.click_gui",
				InputConstants.Type.KEYSYM,
				InputConstants.KEY_RSHIFT,
				CATEGORY
		);
		hudEditorKey = new KeyMapping(
				"key.aether.hud_editor",
				InputConstants.Type.KEYSYM,
				InputConstants.UNKNOWN.getValue(),
				CATEGORY
		);

		KeyBindingHelper.registerKeyBinding(clickGuiKey);
		KeyBindingHelper.registerKeyBinding(hudEditorKey);

		// Every module gets its mapping here, once. Fabric refuses a mapping registered after the
		// options of the game exist (KeyBindingRegistryImpl throws as soon as GameOptions is set),
		// so a keybind created from the click GUI would crash: the mappings are therefore created
		// now, mostly unbound, and the GUI only moves the key of the one it rebinds.
		Map<String, Integer> defaults = Map.of(
				"kill aura", InputConstants.KEY_G,
				"auto crystal", InputConstants.KEY_V,
				"zoom", InputConstants.KEY_C);
		for (Module module : moduleManager.getAll()) {
			String name = module.getName().toLowerCase(java.util.Locale.ROOT);
			module.createKeyBinding(defaults.getOrDefault(name, InputConstants.UNKNOWN.getValue()),
					InputConstants.Type.KEYSYM);
		}
	}

	/**
	 * Registers the callbacks of the client.
	 */
	private void registerEvents() {
		HudRenderCallback.EVENT.register(this::onHudRender);
		ClientTickEvents.END_CLIENT_TICK.register(this::onTick);

		// The window only exists once the client started, the HUD is therefore placed afterwards.
		ClientLifecycleEvents.CLIENT_STARTED.register(client -> {
			hudManager.applyDefaultPositions(RenderUtil.screenWidth(), RenderUtil.screenHeight());
			hudManager.applySavedPositions(Aether.get().getConfig());
			if (recoveryStartup.recoveryRecommended()) {
				Screen previousScreen = client.screen;
				client.setScreen(new ConfirmScreen(
						useSafeMode -> {
							recoveryDecisionMade = true;
							if (useSafeMode) {
								recoverySafeMode = true;
								moduleManager.disableAll();
							}
						client.setScreen(previousScreen);
						},
						Component.literal("Aether recovery mode"),
						Component.literal("Two consecutive sessions did not close normally. This is not proof that Aether caused a crash. Start this session with Aether modules disabled? Your saved preferences will be preserved."),
						Component.literal("Start safe mode"),
						Component.literal("Continue normally")));
			}
		});

		ClientLifecycleEvents.CLIENT_STOPPING.register(client -> {
			hudManager.capturePositions(Aether.get().getConfig());
			// A safe-mode session must never persist its temporary disabled module states.
			if (recoverySafeMode) {
				config.save();
			} else {
				Aether.get().save();
			}
			if (recoveryMarkerInitialized) {
				try {
					ClientRecoveryGuard.finish(recoveryMarker);
				} catch (IOException exception) {
					LOGGER.warn("Could not clear the Aether session marker after an orderly shutdown.", exception);
				}
			}
		});

		ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) -> {
			dispatcher.register(ClientCommandManager.literal("aether")
					.executes(context -> {
						context.getSource().sendFeedback(Component.literal(
								Aether.NAME + " " + Aether.VERSION + " - session: " + SessionUtil.getSessionLabel()));
						return 1;
					})
					.then(ClientCommandManager.literal("safe-mode").executes(context -> {
						if (!configApplied) {
							context.getSource().sendFeedback(Component.literal(
									"Aether is still loading its configuration. Try /aether safe-mode again shortly."));
							return 0;
						}
						if (recoverySafeMode) {
							context.getSource().sendFeedback(Component.literal("Aether safe mode is already active."));
							return 1;
						}
						recoverySafeMode = true;
						moduleManager.disableAll();
						context.getSource().sendFeedback(Component.literal(
								"Aether safe mode is active for this session. Your saved module preferences will be preserved."));
						return 1;
					})));
		});
	}

	/**
	 * Draws the HUD of the client.
	 */
	private void onHudRender(GuiGraphics graphics, net.minecraft.client.DeltaTracker deltaTracker) {
		Minecraft minecraft = Minecraft.getInstance();

		if (minecraft.screen != null) {
			return;
		}

		hudManager.render(graphics, 0.0f);
		moduleManager.render(graphics, 0.0f);
	}

	/**
	 * Runs the tick logic of the client.
	 */
	private void onTick(Minecraft minecraft) {
		if (!recoveryNoticeShown && minecraft.player != null) {
			String recoveryMessage = config.getRecoveryMessage();
			if (recoveryMessage != null) {
				minecraft.player.displayClientMessage(Component.literal(recoveryMessage), false);
			}
			recoveryNoticeShown = true;
		}

		// Do not restore or tick configured modules until the player has explicitly chosen safe
		// recovery or normal startup on the title screen.
		if (recoveryStartup.recoveryRecommended() && !recoveryDecisionMade) {
			return;
		}

		// The modules write into the options of the game, which do not exist yet while the client is
		// being created. The saved state is therefore restored on the first tick: applying it earlier
		// crashed the game as soon as a module touching the options (Fullbright, FOV, No View Bobbing,
		// Auto Jump) was enabled in the configuration.
		if (!configApplied && minecraft.options != null) {
			configApplied = true;
			config.applyTo(moduleManager, !recoverySafeMode);
		}

		while (clickGuiKey.consumeClick()) {
			if (minecraft.screen == null && !recoverySafeMode) {
				minecraft.setScreen(new ClickGuiScreen(ModuleCategory.RENDER));
			}
		}

		while (hudEditorKey.consumeClick()) {
			if (minecraft.screen == null && !recoverySafeMode) {
				minecraft.setScreen(new ConfigScreen());
			}
		}

		// The keybinds of the modules appeared in the controls screen and in the click GUI, but
		// nothing ever read them: pressing G left Kill Aura untouched. The press is consumed once
		// here, and only outside a screen, so that typing in the click GUI cannot toggle a module
		// behind it.
		if (!recoverySafeMode && minecraft.screen == null) {
			boolean toggled = false;
			for (Module module : moduleManager.getAll()) {
				KeyMapping mapping = module.getKeyBinding();
				while (mapping != null && mapping.consumeClick()) {
					module.toggle();
					toggled = true;
				}
			}
			if (toggled) {
				Aether.get().save();
			}
		}

		moduleManager.tick();
		// The switch animations are advanced by the screen that draws them, on the frame clock:
		// a fixed 50 ms step here made them move 20 times per second while the interface painted
		// 60 to 240 frames.
	}

	/**
	 * @return the instance of the client.
	 */
	public static AetherClient getInstance() {
		return instance;
	}
}

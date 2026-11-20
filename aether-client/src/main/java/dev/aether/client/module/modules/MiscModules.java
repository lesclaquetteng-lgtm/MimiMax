package dev.aether.client.module.modules;

import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.IntegerSetting;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.setting.ModeSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;

import java.util.ArrayDeque;
import java.util.Deque;

/**
 * The modules that do not fit into any other category.
 */
public final class MiscModules {
	private MiscModules() {
	}

	/**
	 * Hides the scoreboard of the server.
	 */
	public static class HideScoreboard extends Module {
		public HideScoreboard() {
			super("Hide Scoreboard", "Hides the scoreboard of the server", ModuleCategory.MISC, false);
		}
	}

	/**
	 * Shows the chat messages of the server in the corner of the screen.
	 */
	public static class Notifications extends Module {
		private static final int MAX_MESSAGES = 5;

		private static Notifications instance;

		private final BooleanSetting showGameMessages = register(new BooleanSetting("Game messages", "Also shows the messages of the game", true));
		private final IntegerSetting duration = register(new IntegerSetting("Duration", "How long a message stays visible in seconds", 5, 1, 20));

		private final Deque<Entry> entries = new ArrayDeque<>();

		public Notifications() {
			super("Notifications", "Shows the chat messages in the corner of the screen", ModuleCategory.MISC, false);
			instance = this;
		}

		/**
		 * Called by the mixin when a chat message arrives.
		 * @param message the plain text of the message
		 */
		public static void push(String message) {
			if (instance != null) {
				instance.receive(message);
			}
		}

		private void receive(String message) {
			if (message == null || message.isBlank()) {
				return;
			}
			if (!showGameMessages.isEnabled() && message.startsWith("[")) {
				return;
			}
			entries.addLast(new Entry(message, System.currentTimeMillis()));
			while (entries.size() > MAX_MESSAGES) {
				entries.removeFirst();
			}
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			long now = System.currentTimeMillis();
			long lifetime = duration.get() * 1000L;

			int right = graphics.guiWidth() - 4;
			int y = 6;

			for (Entry entry : entries) {
				long age = now - entry.timestamp;
				if (age > lifetime) {
					continue;
				}

				// The message slides in from the right edge during its first 180 ms, then stays
				// put and fades out during the last 600 ms of its life.
				float in = Math.min(1.0f, age / 180.0f);
				float ease = 1.0f - (1.0f - in) * (1.0f - in);
				float fade = Math.min(ease, Math.min(1.0f, (lifetime - age) / 600.0f));
				int slide = Math.round((1.0f - ease) * 16.0f);

				String text = RenderUtil.truncate(minecraft.font, entry.message, 220);
				int boxWidth = minecraft.font.width(text) + 12;
				int boxX = right - boxWidth + slide;

				RenderUtil.roundedRect(graphics, boxX, y, boxWidth, 14, 4,
						ColorUtil.withAlpha(RenderUtil.PANEL_BACKGROUND, fade));
				RenderUtil.text(graphics, minecraft.font, text, boxX + 6, y + 3,
						ColorUtil.withAlpha(RenderUtil.TEXT_PRIMARY, fade));
				y += 16;
			}
		}

		/**
		 * A single chat message.
		 */
		private static final class Entry {
			private final String message;
			private final long timestamp;

			private Entry(String message, long timestamp) {
				this.message = message;
				this.timestamp = timestamp;
			}
		}
	}

	/**
	 * Sends the chosen message in chat when the player dies.
	 *
	 * <p>The description promised a message at the end of a game. Detecting the end of a match is a
	 * per-server job (a chat pattern, a scoreboard, a title) that this client does not do, so the one
	 * trigger it has — the death of the local player — is the one it now announces.</p>
	 */
	public static class AutoGG extends Module {
		private final ModeSetting phrase = register(new ModeSetting("Message", "Message sent to chat", "gg", "gg", "GG", "gg wp", "Good game!"));
		private boolean triggered;

		public AutoGG() {
			super("Auto GG", "Sends the chosen message when the player dies", ModuleCategory.MISC, false);
		}

		@Override
		public void onTick() {
			Minecraft mc = Minecraft.getInstance();
			if (mc.player == null) {
				triggered = false;
				return;
			}
			if (mc.player.isDeadOrDying() && !triggered) {
				triggered = true;
				if (mc.player.connection != null) {
					mc.player.connection.sendChat(phrase.get());
				}
			} else if (!mc.player.isDeadOrDying() && triggered) {
				triggered = false;
			}
		}
	}

	/**
	 * Periodically performs subtle player movements to prevent idle kicks.
	 */
	public static class AntiAFK extends Module {
		private final IntegerSetting intervalSeconds = register(new IntegerSetting("Interval", "Seconds between anti-idle actions", 30, 5, 120));
		private int ticks;

		public AntiAFK() {
			super("Anti AFK", "Prevents server inactivity kicks", ModuleCategory.MISC, false);
		}

		@Override
		public void onTick() {
			Minecraft mc = Minecraft.getInstance();
			if (mc.player == null) return;

			ticks++;
			int targetTicks = intervalSeconds.get() * 20;
			if (ticks >= targetTicks) {
				ticks = 0;
				mc.player.setYRot(mc.player.getYRot() + 2.0f);
				if (mc.player.onGround()) {
					mc.player.jumpFromGround();
				}
			}
		}

		@Override
		public void onDisable() {
			ticks = 0;
		}
	}
}

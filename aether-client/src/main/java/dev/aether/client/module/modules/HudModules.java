package dev.aether.client.module.modules;

import dev.aether.client.Aether;
import dev.aether.client.module.HudElement;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.ModeSetting;
import dev.aether.client.util.Animation;
import dev.aether.client.util.ColorUtil;
import dev.aether.client.util.BarChart;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.util.SessionUtil;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.core.Holder;
import net.minecraft.network.chat.Component;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.effect.MobEffectCategory;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;

/**
 * The elements the player can place on the HUD.
 *
 * Every element is a module, so it can be enabled, disabled and configured like any other feature.
 */
public final class HudModules {
	private HudModules() {
	}

	/**
	 * Draws the background of a HUD element.
	 */
	static void drawPanel(GuiGraphics graphics, Font font, int x, int y, int width, int height) {
		RenderUtil.bordered(graphics, x, y, width, height, 4,
				RenderUtil.theme().surface(), RenderUtil.theme().border());
	}

	/**
	 * Draws the background of a HUD element with an opacity, for the elements that fade in and out.
	 */
	static void drawPanel(GuiGraphics graphics, Font font, int x, int y, int width, int height, float opacity) {
		RenderUtil.bordered(graphics, x, y, width, height, 4,
				dev.aether.client.util.ColorUtil.fade(RenderUtil.theme().surface(), opacity),
				dev.aether.client.util.ColorUtil.fade(RenderUtil.theme().border(), opacity));
	}

	/** The accent colour of the configuration, used by every element that highlights a value. */
	static int accent() {
		return Aether.get().getConfig().getAccentColor();
	}

	/** The colour of a frame rate: green when fluid, amber when playable, red when struggling. */
	static int fpsColour(int fps) {
		return fps >= 120 ? 0xFF34C759 : fps >= 60 ? 0xFFFFD60A : 0xFFFF453A;
	}

	/**
	 * The frames per second of the game.
	 */
	public static class Fps extends HudElement {
		private final BooleanSetting showMaximum = register(new BooleanSetting("Max", "Shows the highest frame rate of the session", false));
		private final BooleanSetting showAverage = register(new BooleanSetting("Average", "Shows the average frame rate of the session", false));
		private final BooleanSetting coloured = register(new BooleanSetting("Colour", "Colours the value depending on the frame rate", true));

		private int maximum;
		private int average;
		private int samples;
		private long accumulated;

		// The line is rebuilt only when one of its inputs changes, and the measure reads the same
		// cache: a steady frame rate costs no allocation at all.
		private int cachedFps = -1;
		private int cachedMaximum = -1;
		private int cachedAverage = -1;
		private boolean cachedShowMaximum;
		private boolean cachedShowAverage;
		private String text = "";
		private int textWidth;

		public Fps() {
			super("FPS", "Shows the current frame rate", ModuleCategory.HUD, true);
		}

		@Override
		public void onTick() {
			int fps = Minecraft.getInstance().getFps();
			maximum = Math.max(maximum, fps);
			accumulated += fps;
			samples++;
			if (samples >= 100) {
				average = (int) (accumulated / samples);
				accumulated = 0;
				samples = 0;
			}
		}

		private String text() {
			int fps = Minecraft.getInstance().getFps();
			if (fps == cachedFps && maximum == cachedMaximum && average == cachedAverage
					&& showMaximum.isEnabled() == cachedShowMaximum
					&& showAverage.isEnabled() == cachedShowAverage) {
				return text;
			}

			StringBuilder builder = new StringBuilder();
			builder.append(fps).append(" FPS");
			if (showMaximum.isEnabled()) {
				builder.append("  |  ").append(maximum).append(" max");
			}
			if (showAverage.isEnabled()) {
				builder.append("  |  ").append(average).append(" avg");
			}

			text = builder.toString();
			textWidth = Minecraft.getInstance().font.width(text);
			cachedFps = fps;
			cachedMaximum = maximum;
			cachedAverage = average;
			cachedShowMaximum = showMaximum.isEnabled();
			cachedShowAverage = showAverage.isEnabled();
			return text;
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			int colour = coloured.isEnabled() ? fpsColour(cachedFps) : RenderUtil.theme().textPrimary();
			drawPanel(graphics, Minecraft.getInstance().font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, Minecraft.getInstance().font, text, getX() + 6, getY() + 3, colour);
		}

		@Override
		public int measureWidth() {
			text();
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * A graph of the recent frame rate.
	 *
	 * The value of every client tick is kept, so the graph shows the last three seconds. The bar
	 * that leaves the window is replaced: the buffer never grows and the drawing costs one
	 * rectangle per sample.
	 */
	public static class FpsGraph extends HudElement {
		private static final int SAMPLES = 54;
		private static final int WIDTH = 120;
		private static final int HEIGHT = 36;
		/** The width of one sample: 54 × 2 = the 108 px of the chart. */
		private static final int BAR_WIDTH = 2;

		private final int[] history = new int[SAMPLES];
		/** The samples in the order they are drawn, oldest first. */
		private final int[] ordered = new int[SAMPLES];
		/** The runs the chart is drawn from, four ints each: from, to, height, colour. */
		private final int[] runs = new int[SAMPLES * 4];
		private int cursor;
		private int peak = 60;
		private final BooleanSetting showValue = register(new BooleanSetting("Value", "Shows the current frame rate above the graph", true));

		public FpsGraph() {
			super("FPS Graph", "Graphs the frame rate of the last three seconds", ModuleCategory.HUD, false);
		}

		@Override
		public void onTick() {
			int fps = Minecraft.getInstance().getFps();
			history[cursor] = fps;
			cursor = (cursor + 1) % SAMPLES;

			// The peak falls back slowly, so that a short drop is still readable.
			peak = Math.max(60, Math.max(fps, (int) (peak * 0.995)));
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			Font font = minecraft.font;
			int x = getX();
			int y = getY();

			drawPanel(graphics, font, x, y, WIDTH, HEIGHT);

			if (showValue.isEnabled()) {
				String text = minecraft.getFps() + " FPS";
				RenderUtil.text(graphics, font, text, x + 6, y + 4, RenderUtil.theme().textSecondary());
			}

			int graphX = x + 6;
			int graphY = y + 15;
			int graphWidth = SAMPLES * BAR_WIDTH;
			int graphHeight = HEIGHT - 21;

			graphics.fill(graphX, graphY, graphX + graphWidth, graphY + graphHeight, 0x66000000);

			renderBars(graphics, graphX, graphY, graphHeight);
		}

		/**
		 * Draws the chart as one rectangle per run of adjacent samples of the same height and colour:
		 * a steady frame rate costs a handful of rectangles instead of one per sample.
		 */
		private void renderBars(GuiGraphics graphics, int graphX, int graphY, int graphHeight) {
			for (int index = 0; index < SAMPLES; index++) {
				ordered[index] = history[(cursor + index) % SAMPLES];
			}

			int count = BarChart.runs(ordered, peak, graphHeight, HudModules::fpsColour, runs);
			for (int run = 0; run < count; run++) {
				int bar = runs[run * 4 + 2];
				if (bar <= 0) {
					continue;
				}
				int left = graphX + runs[run * 4] * BAR_WIDTH;
				int right = graphX + runs[run * 4 + 1] * BAR_WIDTH;
				graphics.fill(left, graphY + graphHeight - bar, right, graphY + graphHeight, runs[run * 4 + 3]);
			}
		}

		@Override
		public int measureWidth() {
			return WIDTH;
		}

		@Override
		public int measureHeight() {
			return HEIGHT;
		}
	}

	/**
	 * The latency towards the server.
	 */
	public static class Ping extends HudElement {
		private final BooleanSetting showColour = register(new BooleanSetting("Colour", "Colours the value depending on the latency", true));

		private int cachedLatency = -1;
		private String text = "";
		private int textWidth;

		public Ping() {
			super("Ping", "Shows the latency towards the server", ModuleCategory.HUD, true);
		}

		private int getLatency() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.getConnection() == null) {
				return 0;
			}
			PlayerInfo info = minecraft.getConnection().getPlayerInfo(minecraft.getUser().getName());
			return info == null ? 0 : info.getLatency();
		}

		private String text() {
			int latency = getLatency();
			if (latency != cachedLatency) {
				text = latency + " ms";
				textWidth = Minecraft.getInstance().font.width(text);
				cachedLatency = latency;
			}
			return text;
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			Font font = Minecraft.getInstance().font;

			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			int colour = RenderUtil.theme().textPrimary();
			if (showColour.isEnabled()) {
				if (cachedLatency >= 200) {
					colour = 0xFFE05260;
				} else if (cachedLatency >= 100) {
					colour = 0xFFF0B429;
				} else {
					colour = 0xFF2ECC8F;
				}
			}
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, colour);
		}

		@Override
		public int measureWidth() {
			text();
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * The clicks per second of the mouse buttons.
	 */
	public static class Cps extends HudElement {
		private final BooleanSetting showRight = register(new BooleanSetting("Right click", "Also shows the right mouse button", false));

		private int cachedLeft = -1;
		private int cachedRight = -1;
		private boolean cachedShowRight;
		private String text = "";
		private int textWidth;

		public Cps() {
			super("CPS", "Shows the clicks per second of the player", ModuleCategory.HUD, true);
		}

		private String text() {
			int left = Aether.get().getClickTracker().getLeftClicksPerSecond();
			int right = Aether.get().getClickTracker().getRightClicksPerSecond();
			if (left == cachedLeft && right == cachedRight && showRight.isEnabled() == cachedShowRight) {
				return text;
			}

			text = left + " CPS";
			if (showRight.isEnabled()) {
				text += "  |  " + right + " CPS";
			}
			textWidth = Minecraft.getInstance().font.width(text);
			cachedLeft = left;
			cachedRight = right;
			cachedShowRight = showRight.isEnabled();
			return text;
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			Font font = Minecraft.getInstance().font;
			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
		}

		@Override
		public int measureWidth() {
			text();
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * A small overview of the keys the player is pressing.
	 *
	 * <p>Each key fades towards the accent colour instead of snapping, and the mouse key can show
	 * the clicks per second under its label.</p>
	 */
	public static class Keystrokes extends HudElement {
		private static final int KEY_SIZE = 22;
		private static final int GAP = 2;
		private static final int KEY_COUNT = 7;

		private final BooleanSetting showCps = register(new BooleanSetting("CPS", "Shows the clicks per second on the mouse key", true));

		private final Animation[] presses = new Animation[KEY_COUNT];
		private final boolean[] down = new boolean[KEY_COUNT];
		private long lastFrame;

		public Keystrokes() {
			super("Keystrokes", "Shows the keys that are currently pressed", ModuleCategory.HUD, false);
			for (int i = 0; i < KEY_COUNT; i++) {
				presses[i] = new Animation(0.0f, 18.0f);
			}
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			Font font = minecraft.font;
			int x = getX();
			int y = getY();

			down[0] = minecraft.options.keyUp.isDown();
			down[1] = minecraft.options.keyLeft.isDown();
			down[2] = minecraft.options.keyDown.isDown();
			down[3] = minecraft.options.keyRight.isDown();
			down[4] = minecraft.mouseHandler.isLeftPressed();
			down[5] = minecraft.options.keyJump.isDown();
			down[6] = minecraft.options.keyShift.isDown();

			// The fade runs on the frame clock, not the tick clock, so it stays smooth at any
			// frame rate. The delta is clamped so a paused game does not jump the animation.
			long now = System.nanoTime();
			long deltaMillis = lastFrame == 0 ? 16 : Math.min(100, (now - lastFrame) / 1_000_000);
			lastFrame = now;
			for (int i = 0; i < KEY_COUNT; i++) {
				presses[i].setTarget(down[i] ? 1.0f : 0.0f);
				presses[i].update(deltaMillis);
			}

			drawPanel(graphics, font, x - 3, y - 3, KEY_SIZE * 3 + GAP * 2 + 6, KEY_SIZE * 2 + GAP * 2 + 6);

			drawKey(graphics, font, x + KEY_SIZE + GAP, y, "W", presses[0].getValue(), -1);
			drawKey(graphics, font, x, y + KEY_SIZE + GAP, "A", presses[1].getValue(), -1);
			drawKey(graphics, font, x + KEY_SIZE + GAP, y + KEY_SIZE + GAP, "S", presses[2].getValue(), -1);
			drawKey(graphics, font, x + (KEY_SIZE + GAP) * 2, y + KEY_SIZE + GAP, "D", presses[3].getValue(), -1);

			int bottom = y + (KEY_SIZE + GAP) * 2;
			int cps = showCps.isEnabled() ? Aether.get().getClickTracker().getLeftClicksPerSecond() : -1;
			drawKey(graphics, font, x, bottom, "LMB", presses[4].getValue(), cps);
			drawKey(graphics, font, x + KEY_SIZE + GAP, bottom, "SPC", presses[5].getValue(), -1);
			drawKey(graphics, font, x + (KEY_SIZE + GAP) * 2, bottom, "SNK", presses[6].getValue(), -1);
		}

		private void drawKey(GuiGraphics graphics, Font font, int x, int y, String label, float progress, int cps) {
			int accent = Aether.get().getConfig().getAccentColor();
			int background = ColorUtil.lerp(0xC0202530, accent, progress);
			int textColour = ColorUtil.lerp(RenderUtil.theme().textSecondary(), 0xFFFFFFFF, progress);
			RenderUtil.roundedRect(graphics, x, y, KEY_SIZE, KEY_SIZE, 4, background);

			if (cps >= 0) {
				// Two lines: the label on top, the clicks per second below.
				RenderUtil.centeredText(graphics, font, label, x + KEY_SIZE / 2, y + 2, textColour);
				RenderUtil.centeredText(graphics, font, String.valueOf(cps), x + KEY_SIZE / 2, y + 12, textColour);
			} else {
				RenderUtil.centeredText(graphics, font, label, x + KEY_SIZE / 2, y + (KEY_SIZE - 8) / 2, textColour);
			}
		}

		@Override
		public int measureWidth() {
			return KEY_SIZE * 3 + GAP * 2;
		}

		@Override
		public int measureHeight() {
			return KEY_SIZE * 2 + GAP * 2;
		}
	}

	/**
	 * The armour and the off hand of the player.
	 */
	public static class Armor extends HudElement {
		private static final EquipmentSlot[] SLOTS = {
				EquipmentSlot.CHEST, EquipmentSlot.HEAD, EquipmentSlot.LEGS, EquipmentSlot.FEET, EquipmentSlot.OFFHAND
		};

		public Armor() {
			super("Armor", "Shows the equipped armour of the player", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null) {
				return;
			}
			Font font = minecraft.font;

			int x = getX();
			int y = getY();

			drawPanel(graphics, font, x - 3, y - 3, 22 * 5 + 3, 26);

			for (EquipmentSlot slot : SLOTS) {
				ItemStack stack = minecraft.player.getItemBySlot(slot);
				int bg = stack.isEmpty() ? 0x1A141722 : 0x40141722;
				int border = stack.isEmpty() ? 0x203D4560 : RenderUtil.theme().borderStrong();
				RenderUtil.bordered(graphics, x, y, 20, 20, 4, bg, border);
				if (!stack.isEmpty()) {
					graphics.renderItem(stack, x + 2, y + 2);
					if (stack.isDamageableItem() && stack.getMaxDamage() > 0) {
						float dur = 1.0f - (float) stack.getDamageValue() / (float) stack.getMaxDamage();
						int durColor = dur < 0.2f ? 0xFFE05260 : (dur < 0.5f ? 0xFFF0B429 : 0xFF2ECC8F);
						RenderUtil.bar(graphics, x + 2, y + 17, 16, 2, dur, 0x66000000, durColor);
					}
				}
				x += 22;
			}
		}

		@Override
		public int measureWidth() {
			return 22 * 5;
		}

		@Override
		public int measureHeight() {
			return 20;
		}
	}

	/**
	 * The active potion effects of the player.
	 */
	public static class Potions extends HudElement {
		public Potions() {
			super("Potions", "Shows the active potion effects", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null) {
				return;
			}
			Font font = minecraft.font;

			int x = getX();
			int y = getY();
			int count = 0;

			for (MobEffectInstance effect : minecraft.player.getActiveEffects()) {
				Holder<MobEffect> holder = effect.getEffect();
				Component name = holder.value().getDisplayName();
				String label = name.getString();
				if (effect.getAmplifier() > 0) {
					label += " " + toRoman(effect.getAmplifier() + 1);
				}
				label += " " + formatDuration(effect.getDuration());

				// The colour tells the kind of effect at a glance: green helps, red hurts, blue
				// is neutral. A thin strip on the left carries the same colour.
				int colour = effectColour(holder.value().getCategory());
				int width = font.width(label) + 12;
				RenderUtil.roundedRect(graphics, x, y, width, 13, 3, RenderUtil.theme().elevatedBackground());
				graphics.fill(x + 2, y + 3, x + 4, y + 10, colour);
				RenderUtil.text(graphics, font, label, x + 7, y + 2, colour);
				y += 15;
				count++;
			}

			if (count == 0) {
				RenderUtil.roundedRect(graphics, x, y, 52, 13, 3, RenderUtil.theme().elevatedBackground());
				RenderUtil.text(graphics, font, "No effects", x + 5, y + 2, RenderUtil.theme().textDim());
			}
		}

		/** The colour of an effect category, from the palette of the client. */
		private static int effectColour(MobEffectCategory category) {
			return switch (category) {
				case BENEFICIAL -> 0xFF2ECC8F;
				case HARMFUL -> 0xFFE05260;
				default -> 0xFF3498DB;
			};
		}

		private static String toRoman(int number) {
			return switch (number) {
				case 1 -> "I";
				case 2 -> "II";
				case 3 -> "III";
				case 4 -> "IV";
				case 5 -> "V";
				default -> String.valueOf(number);
			};
		}

		private static String formatDuration(int ticks) {
			int seconds = ticks / 20;
			int minutes = seconds / 60;
			int sec = seconds % 60;
			if (minutes >= 60) {
				int hours = minutes / 60;
				int min = minutes % 60;
				return hours + ":" + (min < 10 ? "0" : "") + min + ":" + (sec < 10 ? "0" : "") + sec;
			}
			return minutes + ":" + (sec < 10 ? "0" : "") + sec;
		}

		@Override
		public int measureWidth() {
			return 90;
		}

		@Override
		public int measureHeight() {
			Minecraft minecraft = Minecraft.getInstance();
			int count = minecraft.player == null ? 0 : minecraft.player.getActiveEffects().size();
			return count == 0 ? 13 : count * 15 - 2;
		}
	}

	/**
	 * The position of the player inside the world.
	 */
	public static class Coordinates extends HudElement {
		private final BooleanSetting showFacing = register(new BooleanSetting("Facing", "Shows the direction the player is looking at", true));
		private final BooleanSetting showBiome = register(new BooleanSetting("Biome", "Shows the biome the player is in", true));

		// The line is rebuilt only when the displayed tenth of a block, the facing or the biome
		// changes: standing still costs nothing, and the biome lookup follows the block, not the frame.
		private long cachedX = Long.MIN_VALUE;
		private long cachedY;
		private long cachedZ;
		private int cachedFacing = -1;
		private long cachedBiomeBlock = Long.MIN_VALUE;
		private String cachedBiome = "";
		private boolean cachedShowFacing;
		private boolean cachedShowBiome;
		private String text;
		private int textWidth;

		public Coordinates() {
			super("Coordinates", "Shows the position of the player", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			if (text == null) {
				return;
			}

			Font font = Minecraft.getInstance().font;
			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
		}

		/** @return the line the element shows, or null when there is nothing to describe yet. */
		private String text() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null || minecraft.level == null) {
				text = null;
				return null;
			}

			long x = Math.round(minecraft.player.getX() * 10.0);
			long y = Math.round(minecraft.player.getY() * 10.0);
			long z = Math.round(minecraft.player.getZ() * 10.0);
			int facing = facingIndex(minecraft.player.getYRot());
			long biomeBlock = minecraft.player.blockPosition().asLong();

			if (x == cachedX && y == cachedY && z == cachedZ && facing == cachedFacing
					&& biomeBlock == cachedBiomeBlock
					&& showFacing.isEnabled() == cachedShowFacing
					&& showBiome.isEnabled() == cachedShowBiome) {
				return text;
			}

			if (biomeBlock != cachedBiomeBlock) {
				cachedBiome = getBiomeName(minecraft);
				cachedBiomeBlock = biomeBlock;
			}

			StringBuilder builder = new StringBuilder();
			builder.append(String.format("XYZ %.1f %.1f %.1f", x / 10.0, y / 10.0, z / 10.0));
			if (showFacing.isEnabled()) {
				builder.append("  ").append(facingName(facing));
			}
			if (showBiome.isEnabled()) {
				builder.append("  ").append(cachedBiome);
			}

			text = builder.toString();
			textWidth = minecraft.font.width(text);
			cachedX = x;
			cachedY = y;
			cachedZ = z;
			cachedFacing = facing;
			cachedShowFacing = showFacing.isEnabled();
			cachedShowBiome = showBiome.isEnabled();
			return text;
		}

		/**
		 * @return the name of the biome the player is currently in.
		 */
		private static String getBiomeName(Minecraft minecraft) {
			var holder = minecraft.level.getBiome(minecraft.player.blockPosition());
			return holder.unwrapKey()
					.map(key -> key.identifier().getPath())
					.orElse("unknown");
		}

		private static int facingIndex(float yaw) {
			return Math.floorMod((int) (yaw * 4.0f / 360.0f) + 4, 4);
		}

		private static String facingName(int direction) {
			return switch (direction) {
				case 0 -> "North";
				case 1 -> "East";
				case 2 -> "South";
				default -> "West";
			};
		}

		@Override
		public int measureWidth() {
			return text() == null ? 90 : textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * The brand of the server the player is playing on.
	 */
	public static class ServerBrand extends HudElement {
		private String cachedBrand;
		private String text;
		private int textWidth;

		public ServerBrand() {
			super("Server Brand", "Shows the software the server runs on", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			if (text == null) {
				return;
			}

			Font font = Minecraft.getInstance().font;
			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
		}

		/** @return the software of the server, or null when the player is not connected. */
		private String text() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.getConnection() == null) {
				text = null;
				return null;
			}
			String brand = minecraft.getConnection().serverBrand();
			if (text == null || brand != cachedBrand) {
				text = brand == null || brand.isBlank() ? "vanilla" : brand;
				textWidth = minecraft.font.width(text);
				cachedBrand = brand;
			}
			return text;
		}

		@Override
		public int measureWidth() {
			return text() == null ? 60 : textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * The item the player is currently holding.
	 */
	public static class HeldItem extends HudElement {
		// The held stack keeps its identity while the player holds it, so the name is read again
		// only when the hand actually changes.
		private ItemStack cachedStack;
		private String text = "Empty";
		private int textWidth = -1;

		public HeldItem() {
			super("Held Item", "Shows the item in the main hand", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			Font font = Minecraft.getInstance().font;
			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
		}

		/** @return the name of the item in the main hand, or Empty. */
		private String text() {
			Minecraft minecraft = Minecraft.getInstance();
			ItemStack stack = minecraft.player == null ? ItemStack.EMPTY : minecraft.player.getMainHandItem();
			if (stack != cachedStack || textWidth < 0) {
				text = stack.isEmpty() ? "Empty" : stack.getHoverName().getString();
				textWidth = minecraft.font.width(text);
				cachedStack = stack;
			}
			return text;
		}

		@Override
		public int measureWidth() {
			text();
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * How long the player has been playing on the current server.
	 */
	public static class SessionTimer extends HudElement {
		private long startTime;
		private boolean running;

		private long cachedSecond = -1;
		private String text = "00:00:00";
		private int textWidth = -1;

		public SessionTimer() {
			super("Session Timer", "Shows how long the current session lasts", ModuleCategory.HUD, false);
		}

		@Override
		public void onEnable() {
			startTime = System.currentTimeMillis();
			running = true;
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			String text = text();
			Font font = Minecraft.getInstance().font;
			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
		}

		private String text() {
			long elapsed = running ? System.currentTimeMillis() - startTime : 0;
			long second = elapsed / 1000;
			if (second != cachedSecond || textWidth < 0) {
				text = String.format("%02d:%02d:%02d", second / 3600, (second / 60) % 60, second % 60);
				textWidth = Minecraft.getInstance().font.width(text);
				cachedSecond = second;
			}
			return text;
		}

		@Override
		public int measureWidth() {
			text();
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	/**
	 * Information about the entity the player is looking at.
	 *
	 * <p>The health bar glides towards the real health instead of jumping, and its colour travels
	 * from red to green with it.</p>
	 */

	public static class TargetHud extends HudElement {
		private final Animation health = new Animation(1.0f, 12.0f);
		private final Animation fadeAnim = new Animation(0.0f, 16.0f);
		/** Set to one when the target loses health, then decays: the flash is a fade, not a cut. */
		private final Animation flash = new Animation(0.0f, 9.0f);
		private long lastFrame;

		private Entity cachedEntity;
		private String cachedName = "";
		private float cachedHealth = -1.0f;
		private float cachedMaxHealth = -1.0f;
		private String cachedHealthText = "";

		public TargetHud() {
			super("Target HUD", "Shows the entity the player is aiming at", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			Font font = minecraft.font;
			boolean hasTarget = (minecraft.hitResult != null
					&& minecraft.hitResult.getType() == HitResult.Type.ENTITY
					&& ((EntityHitResult) minecraft.hitResult).getEntity() != null);

			long now = System.nanoTime();
			long deltaMillis = lastFrame == 0 ? 16 : Math.min(100, (now - lastFrame) / 1_000_000);
			lastFrame = now;

			fadeAnim.setTarget(hasTarget ? 1.0f : 0.0f);
			fadeAnim.update(deltaMillis);
			flash.update(deltaMillis);

			float opacity = fadeAnim.getValue();
			if (opacity <= 0.01f) {
				return;
			}

			Entity entity = hasTarget ? ((EntityHitResult) minecraft.hitResult).getEntity() : cachedEntity;
			if (entity == null) {
				return;
			}

			int x = getX();
			int y = getY();
			int width = 120;
			int height = 34;
			int accent = accent();

			// The whole card fades with the animation: it used to pop in and out at the threshold,
			// because the value was only used as a gate.
			drawPanel(graphics, font, x, y, width, height, opacity);
			RenderUtil.roundedRect(graphics, x + 1, y + 7, 2, 8, 1, ColorUtil.fade(accent, opacity));
			if (entity != cachedEntity) {
				cachedName = RenderUtil.truncate(font, entity.getName().getString(), width - 20);
				cachedEntity = entity;
			}
			RenderUtil.text(graphics, font, cachedName, x + 8, y + 6, ColorUtil.fade(RenderUtil.theme().textPrimary(), opacity));

			if (entity instanceof LivingEntity living) {
				float max = living.getMaxHealth();
				float fraction = max <= 0 ? 0.0f : Math.max(0.0f, Math.min(1.0f, living.getHealth() / max));

				if (cachedHealth > 0 && living.getHealth() < cachedHealth) {
					flash.set(1.0f);
					flash.setTarget(0.0f);
				}

				health.setTarget(fraction);
				health.update(deltaMillis);

				if (living.getHealth() != cachedHealth || max != cachedMaxHealth) {
					cachedHealthText = String.format(java.util.Locale.ROOT, "%.1f / %.1f", living.getHealth(), max);
					cachedHealth = living.getHealth();
					cachedMaxHealth = max;
				}
				RenderUtil.text(graphics, font, cachedHealthText, x + 8, y + 18, ColorUtil.fade(RenderUtil.theme().textSecondary(), opacity));

				int barColour = ColorUtil.lerp(healthColour(health.getValue()), 0xFFFFFFFF, flash.getValue());
				RenderUtil.bar(graphics, x + 8, y + 28, width - 16, 4, health.getValue(),
						ColorUtil.fade(0x40FFFFFF, opacity), ColorUtil.fade(barColour, opacity));
			}
		}

		/** Red when nearly dead, amber at half, green when full. */
		private static int healthColour(float fraction) {
			return fraction < 0.5f
					? ColorUtil.lerp(0xFFE05260, 0xFFF0B429, fraction * 2.0f)
					: ColorUtil.lerp(0xFFF0B429, 0xFF2ECC8F, (fraction - 0.5f) * 2.0f);
		}

		@Override
		public int measureWidth() {
			return 120;
		}

		@Override
		public int measureHeight() {
			return 34;
		}
	}


	// ──────────────────────────────────────────────────────────────────────
	// Speed — player ground speed in m/s
	// ──────────────────────────────────────────────────────────────────────

	/**
	 * The horizontal speed of the player in metres per second.
	 *
	 * <p>The colour is green when sprinting, amber when walking, red when slower.</p>
	 */

	public static class Speed extends HudElement {
		/** The value glides: a player who starts sprinting sees the number climb, not jump. */
		private final Animation smooth = new Animation(0.0f, 16.0f);
		private long lastFrame;
		private float cachedShown = -1.0f;
		private String text = "0.00 m/s";
		private int textWidth;

		public Speed() {
			super("Speed", "Shows the horizontal speed of the player in m/s", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null) {
				return;
			}
			Font font = minecraft.font;

			double dx = minecraft.player.getDeltaMovement().x;
			double dz = minecraft.player.getDeltaMovement().z;
			double speed = Math.sqrt(dx * dx + dz * dz) * 20.0; // blocks/tick → blocks/s

			long now = System.nanoTime();
			long deltaMillis = lastFrame == 0 ? 16 : Math.min(100, (now - lastFrame) / 1_000_000);
			lastFrame = now;
			smooth.setTarget((float) speed);
			smooth.update(deltaMillis);

			float shown = smooth.getValue();
			if (Math.abs(shown - cachedShown) > 0.005f) {
				text = String.format(java.util.Locale.ROOT, "%.2f m/s", shown);
				textWidth = font.width(text);
				cachedShown = shown;
			}

			int colour = shown > 4.3f ? 0xFF2ECC8F : shown > 3.0f ? 0xFFFFD60A : 0xFFE05260;
			drawPanel(graphics, font, getX(), getY(), textWidth + 16, 14);
			// A drawn dot instead of an emoji: the font of the game has no glyph for the bolt, so
			// the old prefix was painted as an empty square.
			RenderUtil.roundedRect(graphics, getX() + 5, getY() + 6, 3, 3, 1, colour);
			RenderUtil.text(graphics, font, text, getX() + 11, getY() + 3, colour);
		}

		@Override
		public int measureWidth() {
			if (textWidth == 0) {
				textWidth = Minecraft.getInstance().font.width(text);
			}
			return textWidth + 16;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}


	// ──────────────────────────────────────────────────────────────────────
	// ComboCounter — consecutive hit counter
	// ──────────────────────────────────────────────────────────────────────

	/**
	 * Counts consecutive hits against entities.
	 *
	 * <p>The counter resets when the player takes damage or when three seconds
	 * pass without a hit.</p>
	 */

	public static class ComboCounter extends HudElement {
		/** Incremented by the mouse mixin via {@link #recordHit()}. */
		private static int combo;
		private static long lastHitTime;

		/** The timeout after which the combo resets, in milliseconds. */
		private static final long TIMEOUT_MS = 3_000L;
		private static final String LABEL = "COMBO \u00d7";

		/** Brightness of the number right after a hit, which decays over a few frames. */
		private final Animation pop = new Animation(0.0f, 12.0f);
		private long lastFrame;
		private int cachedCombo = Integer.MIN_VALUE;
		private int textWidth;
		/** The health seen on the previous tick, so a hit taken ends the streak. */
		private float lastHealth = -1.0f;

		public ComboCounter() {
			super("Combo", "Shows consecutive hits on enemies", ModuleCategory.HUD, false);
		}

		/** Called by the mouse injection so the counter stays in sync. */
		public static void recordHit() {
			combo++;
			lastHitTime = System.currentTimeMillis();
		}

		/** Called when the player takes damage so the streak ends immediately. */
		public static void resetCombo() {
			combo = 0;
			lastHitTime = 0;
		}

		@Override
		public void onTick() {
			if (combo > 0 && System.currentTimeMillis() - lastHitTime > TIMEOUT_MS) {
				resetCombo();
			}

			// Taking damage ends the streak at once: the loss of health is read from the player
			// instead of a hook on the damage path, which keeps the client out of the combat code.
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player != null) {
				float health = minecraft.player.getHealth();
				if (lastHealth > 0.0f && health < lastHealth - 0.01f) {
					resetCombo();
				}
				lastHealth = health;
			}
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Font font = Minecraft.getInstance().font;
			int labelWidth = font.width(LABEL);

			long now = System.nanoTime();
			long deltaMillis = lastFrame == 0 ? 16 : Math.min(100, (now - lastFrame) / 1_000_000);
			lastFrame = now;

			if (combo != cachedCombo) {
				textWidth = labelWidth + font.width(String.valueOf(Math.max(0, combo)));
				cachedCombo = combo;
				if (combo > 0) {
					// A hit flashes the number white for a moment, then it settles back on the accent.
					pop.set(1.0f);
					pop.setTarget(0.0f);
				}
			}
			pop.update(deltaMillis);

			int accent = accent();
			int numberColour = combo > 0
					? ColorUtil.lerp(accent, 0xFFFFFFFF, pop.getValue() * 0.7f)
					: RenderUtil.theme().textDim();

			drawPanel(graphics, font, getX(), getY(), textWidth + 16, 14);
			RenderUtil.roundedRect(graphics, getX() + 5, getY() + 6, 3, 3, 1, combo > 0 ? accent : RenderUtil.theme().textDim());
			RenderUtil.text(graphics, font, LABEL, getX() + 11, getY() + 3, RenderUtil.theme().textSecondary());
			RenderUtil.text(graphics, font, String.valueOf(Math.max(0, combo)),
					getX() + 11 + labelWidth, getY() + 3, numberColour);

			if (combo > 0) {
				// A hairline under the number shows how much of the three seconds is left: the
				// streak ends when it empties, and the player can see it coming.
				long elapsed = System.currentTimeMillis() - lastHitTime;
				float remaining = Math.max(0.0f, Math.min(1.0f, 1.0f - (float) elapsed / TIMEOUT_MS));
				RenderUtil.bar(graphics, getX() + 11, getY() + 11, Math.max(8, textWidth - labelWidth), 2,
						remaining, 0x40FFFFFF, accent);
			}
		}

		@Override
		public int measureWidth() {
			if (textWidth == 0) {
				Font font = Minecraft.getInstance().font;
				textWidth = font.width(LABEL) + font.width("0");
			}
			return textWidth + 16;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}


	// ──────────────────────────────────────────────────────────────────────
	// ReachDisplay — distance to the targeted entity
	// ──────────────────────────────────────────────────────────────────────

	/**
	 * Distance from the player to the entity under the crosshair.
	 */

	public static class ReachDisplay extends HudElement {
		/** The distance glides as well, so walking towards a target does not flicker its decimals. */
		private final Animation smooth = new Animation(0.0f, 16.0f);
		private long lastFrame;
		private boolean hadTarget;
		private float cachedShown = -1.0f;
		private String text = "\u2014";
		private int textWidth;

		public ReachDisplay() {
			super("Reach", "Shows the distance to the targeted entity", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			Font font = minecraft.font;

			boolean hasTarget = minecraft.player != null
					&& minecraft.hitResult != null
					&& minecraft.hitResult.getType() == HitResult.Type.ENTITY;

			long now = System.nanoTime();
			long deltaMillis = lastFrame == 0 ? 16 : Math.min(100, (now - lastFrame) / 1_000_000);
			lastFrame = now;

			if (hasTarget) {
				Entity target = ((EntityHitResult) minecraft.hitResult).getEntity();
				float distance = (float) minecraft.player.distanceTo(target);
				if (!hadTarget) {
					// A new target starts from its real distance instead of sliding from the last one.
					smooth.set(distance);
				}
				smooth.setTarget(distance);
				smooth.update(deltaMillis);

				float shown = smooth.getValue();
				if (Math.abs(shown - cachedShown) > 0.005f) {
					text = String.format(java.util.Locale.ROOT, "%.2f blocks", shown);
					textWidth = font.width(text);
					cachedShown = shown;
				}
			} else if (!text.equals("\u2014")) {
				text = "\u2014";
				textWidth = font.width(text);
				cachedShown = -1.0f;
			}
			hadTarget = hasTarget;

			int colour = hasTarget ? accent() : RenderUtil.theme().textDim();
			drawPanel(graphics, font, getX(), getY(), textWidth + 16, 14);
			RenderUtil.roundedRect(graphics, getX() + 5, getY() + 6, 3, 3, 1, colour);
			RenderUtil.text(graphics, font, text, getX() + 11, getY() + 3, colour);
		}

		@Override
		public int measureWidth() {
			if (textWidth == 0) {
				textWidth = Minecraft.getInstance().font.width(text);
			}
			return textWidth + 16;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}


	// ──────────────────────────────────────────────────────────────────────
	// Direction — compass showing cardinal direction and yaw
	// ──────────────────────────────────────────────────────────────────────

	/**
	 * The direction the player is facing as a cardinal point and degrees.
	 */
	public static class Direction extends HudElement {
		private float cachedYaw = Float.NaN;
		private String text = "";
		private int textWidth;

		public Direction() {
			super("Direction", "Shows the cardinal direction and yaw of the player", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null) {
				return;
			}
			Font font = minecraft.font;

			float yaw = minecraft.player.getYRot() % 360.0f;
			if (yaw < 0) yaw += 360.0f;

			if (Math.abs(yaw - cachedYaw) > 0.5f) {
				String cardinal = cardinalName(yaw);
				text = String.format(java.util.Locale.ROOT, "%s  %.0f\u00b0", cardinal, yaw);
				textWidth = font.width(text);
				cachedYaw = yaw;
			}

			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
		}

		private static String cardinalName(float yaw) {
			// Minecraft yaw: 0 = south, 90 = west, 180 = north, 270 = east
			// Convert so that 0 = north for a compass display
			int index = (int) Math.floor((yaw + 22.5f) / 45.0f) % 8;
			return switch (index) {
				case 0 -> "S";
				case 1 -> "SW";
				case 2 -> "W";
				case 3 -> "NW";
				case 4 -> "N";
				case 5 -> "NE";
				case 6 -> "E";
				default -> "SE";
			};
		}

		@Override
		public int measureWidth() {
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}

	// ──────────────────────────────────────────────────────────────────────
	// Clock — real-world time
	// ──────────────────────────────────────────────────────────────────────

	/**
	 * The current real-world time.
	 */
	public static class Clock extends HudElement {
		private final BooleanSetting showSeconds = register(new BooleanSetting("Seconds", "Shows seconds in the clock", false));

		private String cachedTime = "";
		private String text = "";
		private int textWidth;

		public Clock() {
			super("Clock", "Shows the current real-world time", ModuleCategory.HUD, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Font font = Minecraft.getInstance().font;

			java.time.LocalTime now = java.time.LocalTime.now();
			String timeStr = showSeconds.isEnabled()
					? String.format("%02d:%02d:%02d", now.getHour(), now.getMinute(), now.getSecond())
					: String.format("%02d:%02d", now.getHour(), now.getMinute());

			if (!timeStr.equals(cachedTime)) {
				text = timeStr;
				textWidth = font.width(text);
				cachedTime = timeStr;
			}

			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
			RenderUtil.text(graphics, font, text, getX() + 6, getY() + 3, accent());
		}

		@Override
		public int measureWidth() {
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return 14;
		}
	}


	public static class Session extends HudElement {
		private final ModeSetting layout = register(new ModeSetting("Layout", "How the session is displayed", "Vertical", "Vertical", "Horizontal"));

		// The account does not change during a session: the names and their widths are read once.
		private String name;
		private String type;
		private int nameWidth;
		private int typeWidth;
		private boolean cachedHorizontal;
		private int textWidth;

		public Session() {
			super("Session", "Shows the account and the session type", ModuleCategory.HUD, false);
		}

		private void read() {
			if (name == null) {
				name = SessionUtil.getUsername();
				type = SessionUtil.getSessionLabel();
				Font font = Minecraft.getInstance().font;
				nameWidth = font.width(name);
				typeWidth = font.width(type);
			}
			boolean horizontal = layout.get().equals("Horizontal");
			if (name != null && (textWidth == 0 || horizontal != cachedHorizontal)) {
				Font font = Minecraft.getInstance().font;
				textWidth = horizontal
						? nameWidth + font.width("  ·  ") + typeWidth
						: Math.max(nameWidth, font.width(type + " session"));
				cachedHorizontal = horizontal;
			}
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			read();
			Font font = Minecraft.getInstance().font;
			int typeColour = type.equals("Offline") ? 0xFFF0B429 : 0xFF2ECC8F;

			if (cachedHorizontal) {
				drawPanel(graphics, font, getX(), getY(), textWidth + 12, 14);
				RenderUtil.text(graphics, font, name, getX() + 6, getY() + 3, RenderUtil.theme().textPrimary());
				RenderUtil.text(graphics, font, "  ·  " + type, getX() + 6 + nameWidth, getY() + 3, typeColour);
				return;
			}

			drawPanel(graphics, font, getX(), getY(), textWidth + 12, 28);
			RenderUtil.text(graphics, font, name, getX() + 6, getY() + 4, RenderUtil.theme().textPrimary());
			RenderUtil.text(graphics, font, type + " session", getX() + 6, getY() + 15, typeColour);
		}

		@Override
		public int measureWidth() {
			read();
			return textWidth + 12;
		}

		@Override
		public int measureHeight() {
			return layout.get().equals("Horizontal") ? 14 : 28;
		}
	}

	/**
	 * Displays JVM RAM memory consumption.
	 */
	public static class Memory extends HudElement {
		private int percent;
		private String text = "RAM: 0%";
		private int textWidth;

		public Memory() {
			super("Memory", "Displays RAM memory usage percentage", ModuleCategory.HUD, false);
		}

		@Override
		public void onTick() {
			Runtime runtime = Runtime.getRuntime();
			long max = runtime.maxMemory();
			long total = runtime.totalMemory();
			long free = runtime.freeMemory();
			long used = total - free;
			percent = (int) ((used * 100L) / Math.max(1L, max));
			text = "RAM: " + percent + "%";
			textWidth = Minecraft.getInstance().font.width(text);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Font font = Minecraft.getInstance().font;
			int x = getX();
			int y = getY();
			int w = measureWidth();
			int h = measureHeight();

			drawPanel(graphics, font, x, y, w, h);
			int color = percent > 85 ? 0xFFFF453A : percent > 65 ? 0xFFFFD60A : 0xFF34C759;
			RenderUtil.text(graphics, font, text, x + 6, y + 4, color);
		}

		@Override
		public int measureWidth() {
			return Math.max(58, textWidth + 12);
		}

		@Override
		public int measureHeight() {
			return 16;
		}
	}
}

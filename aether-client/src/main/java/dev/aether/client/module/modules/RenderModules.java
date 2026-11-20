package dev.aether.client.module.modules;

import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.DoubleSetting;
import dev.aether.client.setting.IntegerSetting;
import dev.aether.client.util.EspTargets;
import dev.aether.client.util.RenderUtil;
import dev.aether.client.setting.ModeSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Player;

import java.util.LinkedHashSet;
import java.util.Set;

/**
 * The modules that change the way the game is drawn.
 */
public final class RenderModules {
	private RenderModules() {
	}

	/**
	 * Removes the darkness of the world.
	 */
	public static class Fullbright extends Module {
		private final BooleanSetting gamma = register(new BooleanSetting("Gamma", "Uses the gamma setting instead of a night vision effect", true));

		public Fullbright() {
			super("Fullbright", "Removes the darkness of the world", ModuleCategory.RENDER, false);
		}

		@Override
		public void onEnable() {
			Minecraft minecraft = Minecraft.getInstance();
			if (gamma.isEnabled()) {
				minecraft.options.gamma().set(1.0);
			}
		}

		@Override
		public void onDisable() {
			Minecraft.getInstance().options.gamma().set(0.5);
		}
	}

	/**
	 * Changes the field of view of the game.
	 */
	public static class Fov extends Module {
		private final IntegerSetting value = register(new IntegerSetting("Value", "The field of view of the player", 90, 30, 170));

		public Fov() {
			super("FOV", "Changes the field of view", ModuleCategory.RENDER, false);
		}

		@Override
		public void onEnable() {
			Minecraft.getInstance().options.fov().set(value.get());
		}

		@Override
		public void onDisable() {
			Minecraft.getInstance().options.fov().set(70);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.options.fov().get() != value.get()) {
				minecraft.options.fov().set(value.get());
			}
		}
	}

	/**
	 * Stops the camera from moving while the player walks.
	 */
	public static class NoViewBobbing extends Module {
		public NoViewBobbing() {
			super("No View Bobbing", "Stops the camera from moving while walking", ModuleCategory.RENDER, false);
		}

		@Override
		public void onEnable() {
			Minecraft.getInstance().options.bobView().set(false);
		}

		@Override
		public void onDisable() {
			Minecraft.getInstance().options.bobView().set(true);
		}
	}

	/**
	 * Replaces the crosshair of the game with a custom one.
	 */
	public static class Crosshair extends Module {
		private final IntegerSetting size = register(new IntegerSetting("Size", "The length of the lines of the crosshair", 6, 1, 20));
		private final IntegerSetting thickness = register(new IntegerSetting("Thickness", "The thickness of the lines", 1, 1, 5));
		private final BooleanSetting centerDot = register(new BooleanSetting("Center dot", "Draws a dot in the middle of the screen", false));

		public Crosshair() {
			super("Crosshair", "Replaces the crosshair of the game", ModuleCategory.RENDER, false);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.options.hideGui || minecraft.screen != null) {
				return;
			}

			int centerX = graphics.guiWidth() / 2;
			int centerY = graphics.guiHeight() / 2;
			int length = size.get();
			int thick = thickness.get();
			int colour = Aether.get().getConfig().getAccentColor();

			graphics.fill(centerX - length, centerY, centerX - length + thick, centerY + thick, colour);
			graphics.fill(centerX + length - thick, centerY, centerX + length, centerY + thick, colour);
			graphics.fill(centerX, centerY - length, centerX + thick, centerY - length + thick, colour);
			graphics.fill(centerX, centerY + length - thick, centerX + thick, centerY + length, colour);

			if (centerDot.isEnabled()) {
				graphics.fill(centerX, centerY, centerX + thick, centerY + thick, colour);
			}
		}
	}

	/**
	 * Hides the fire overlay of the screen.
	 *
	 * <p>The flames are not redrawn by the client: {@code ScreenEffectRendererMixin} cancels the
	 * vanilla {@code renderFire} at its head, so the block and water overlays that share the same
	 * renderer keep working.</p>
	 */
	public static class HideFireOverlay extends Module {
		public HideFireOverlay() {
			super("Hide Fire Overlay", "Hides the flames drawn over the screen while burning", ModuleCategory.RENDER, false);
		}
	}

	/**
	 * Hides the camera tilt the game applies when the player takes damage.
	 *
	 * <p>Minecraft 1.21 draws no red texture over the screen for a hit: the feedback is the tilt of
	 * the camera. {@code GameRendererMixin} cancels {@code GameRenderer#bobHurt}, the method that
	 * applies it. Writing into the vanilla {@code damageTiltStrength} option was the first attempt
	 * and it was wrong: that option is saved in {@code options.txt}, so the value read as "before"
	 * on the next launch was the zero the module had written itself.</p>
	 */
	public static class HideHurtOverlay extends Module {
		public HideHurtOverlay() {
			super("Hide Hurt Overlay", "Hides the camera tilt the game applies on a hit", ModuleCategory.RENDER, false);
		}
	}

	/**
	 * Draws a small watermark in the corner of the screen.
	 *
	 * <p>The name carries the accent colour and the version stays discreet, the same two-tone
	 * signature as the sidebar of the click GUI.</p>
	 */
	public static class Watermark extends Module {
		private int nameWidth = -1;
		private int versionWidth;

		public Watermark() {
			super("Watermark", "Shows the name and the version of the client", ModuleCategory.RENDER, true);
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			Minecraft minecraft = Minecraft.getInstance();
			if (nameWidth < 0) {
				nameWidth = minecraft.font.width(Aether.NAME);
				versionWidth = minecraft.font.width(" " + Aether.VERSION);
			}
			int width = nameWidth + versionWidth + 18;

			int x = graphics.guiWidth() - width - 6;
			int y = 6;
			RenderUtil.card(graphics, x, y, width, 16, 5);
			// Status indicator dot with cyber-accent
			int accent = Aether.get().getConfig().getAccentColor();
			RenderUtil.roundedRect(graphics, x + 5, y + 6, 4, 4, 2, accent);
			RenderUtil.text(graphics, minecraft.font, Aether.NAME, x + 13, y + 4, accent);
			RenderUtil.text(graphics, minecraft.font, " " + Aether.VERSION, x + 13 + nameWidth, y + 4,
					RenderUtil.TEXT_MUTED);
		}
	}

	/**
	 * Highlights entities through walls.
	 *
	 * <p>The glow is client state that nothing clears by itself, so the module remembers which
	 * entities it tagged: an entity that leaves the range is released, and turning the module off
	 * releases only its own entities — the ones the server made glow (a spectral arrow, an effect)
	 * keep their glow.</p>
	 */
	public static class Esp extends Module {
		private final ModeSetting targets = register(new ModeSetting("Targets", "Target entities to highlight", "Players", "Players", "Mobs", "All"));

		/** The entities this module is currently keeping glowing. */
		private final Set<Integer> glowing = new LinkedHashSet<>();

		/** The level the ids in {@link #glowing} belong to, null when there is none. */
		private ClientLevel trackedLevel;

		public Esp() {
			super("ESP", "Highlights players and entities through blocks", ModuleCategory.RENDER, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.level == null || minecraft.player == null) {
				unmark(minecraft, EspTargets.stale(glowing, Set.of()));
				glowing.clear();
				trackedLevel = null;
				return;
			}

			if (minecraft.level != trackedLevel) {
				// An entity id is only unique inside one level. Releasing the previous ids after a
				// dimension change would clear the glow of an unrelated entity that happens to reuse
				// one of them, and the entities of the old level are gone with it: the set is dropped
				// instead of being unmarked.
				glowing.clear();
				trackedLevel = minecraft.level;
			}

			Set<Integer> current = new LinkedHashSet<>();
			for (Entity entity : minecraft.level.entitiesForRendering()) {
				if (entity == minecraft.player || !(entity instanceof LivingEntity)) {
					continue;
				}
				if (!EspTargets.selects(targets.get(), entity instanceof Player, entity.distanceTo(minecraft.player))) {
					continue;
				}
				entity.setGlowingTag(true);
				current.add(entity.getId());
			}

			unmark(minecraft, EspTargets.stale(glowing, current));
			glowing.clear();
			glowing.addAll(current);
		}

		@Override
		public void onDisable() {
			Minecraft minecraft = Minecraft.getInstance();
			unmark(minecraft, EspTargets.stale(glowing, Set.of()));
			glowing.clear();
		}

		/** Removes the glow this module added to the given entities. */
		private void unmark(Minecraft minecraft, Set<Integer> ids) {
			if (minecraft.level == null) {
				return;
			}
			for (int id : ids) {
				Entity entity = minecraft.level.getEntity(id);
				if (entity != null) {
					entity.setGlowingTag(false);
				}
			}
		}
	}

	/**
	 * Removes blindness and darkness visual impairments.
	 */
	public static class AntiBlind extends Module {
		private final BooleanSetting darkness = register(new BooleanSetting("Darkness", "Removes warden darkness effect", true));

		public AntiBlind() {
			super("Anti Blind", "Neutralizes blindness and darkness effects", ModuleCategory.RENDER, false);
		}

		@Override
		public void onTick() {
			Minecraft mc = Minecraft.getInstance();
			if (mc.player != null) {
				if (mc.player.hasEffect(MobEffects.BLINDNESS)) {
					mc.player.removeEffect(MobEffects.BLINDNESS);
				}
				if (darkness.isEnabled() && mc.player.hasEffect(MobEffects.DARKNESS)) {
					mc.player.removeEffect(MobEffects.DARKNESS);
				}
			}
		}
	}

	/**
	 * Zooms the view while it is enabled.
	 *
	 * <p>The module used to write the field of view into the vanilla option and put back a literal
	 * value when disabled. That option is saved in {@code options.txt}: quitting with the zoom on
	 * left the zoomed value on disk, so the next launch started zoomed and disabling restored the
	 * wrong number. {@code GameRendererMixin} divides the value returned by
	 * {@code GameRenderer#getFov} instead, which changes the camera for one frame and nothing else.</p>
	 */
	public static class Zoom extends Module {
		private final DoubleSetting factor = register(new DoubleSetting("Zoom Factor", "Magnification multiplier", 3.0, 1.5, 6.0));

		public Zoom() {
			super("Zoom", "Zooms the view while it is enabled", ModuleCategory.RENDER, false);
		}

		/**
		 * @return the divisor the mixin applies to the field of view
		 */
		public double getZoomFactor() {
			return factor.get();
		}
	}

}

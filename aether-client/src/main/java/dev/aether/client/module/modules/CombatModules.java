package dev.aether.client.module.modules;

import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.DoubleSetting;
import dev.aether.client.setting.IntegerSetting;
import dev.aether.client.util.KnockbackReduction;
import dev.aether.client.util.RenderUtil;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.boss.enderdragon.EndCrystal;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

import java.util.Comparator;

/**
 * The modules that help the player during a fight.
 */
public final class CombatModules {
	private CombatModules() {
	}

	/**
	 * Switches to the tool that fits the block the player is mining.
	 */
	public static class AutoTool extends Module {
		private final BooleanSetting switchBack = register(new BooleanSetting("Switch back", "Switches back to the previous item after mining", true));

		private int previousSlot = -1;

		public AutoTool() {
			super("Auto Tool", "Switches to the best tool for the block that is mined", ModuleCategory.COMBAT, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null || minecraft.hitResult == null) {
				return;
			}
			if (!(minecraft.hitResult instanceof net.minecraft.world.phys.BlockHitResult blockHit)) {
				return;
			}

			int selected = minecraft.player.getInventory().getSelectedSlot();
			// Only while the attack key is held: the module used to switch on any block under the
			// crosshair, so simply aiming at dirt changed the item in hand.
			if (!minecraft.options.keyAttack.isDown()) {
				if (switchBack.isEnabled() && previousSlot >= 0) {
					minecraft.player.getInventory().setSelectedSlot(previousSlot);
					previousSlot = -1;
				}
				return;
			}

			BlockState state = minecraft.level.getBlockState(blockHit.getBlockPos());
			int bestSlot = -1;
			float bestSpeed = 0.0f;

			for (int slot = 0; slot < minecraft.player.getInventory().getContainerSize(); slot++) {
				ItemStack stack = minecraft.player.getInventory().getItem(slot);
				if (stack.isEmpty()) {
					continue;
				}
				float speed = stack.getDestroySpeed(state);
				if (speed > bestSpeed) {
					bestSpeed = speed;
					bestSlot = slot;
				}
			}

			if (bestSlot >= 0 && bestSlot < minecraft.player.getInventory().getSelectionSize() && bestSlot != selected) {
				if (previousSlot < 0) {
					previousSlot = selected;
				}
				minecraft.player.getInventory().setSelectedSlot(bestSlot);
			} else if (switchBack.isEnabled() && previousSlot >= 0 && selected != previousSlot) {
				minecraft.player.getInventory().setSelectedSlot(previousSlot);
				previousSlot = -1;
			}
		}

		@Override
		public void onDisable() {
			previousSlot = -1;
		}
	}

	/**
	 * Attacks the closest enemy while the attack key is held.
	 */
	public static class KillAura extends Module {
		private final DoubleSetting range = register(new DoubleSetting("Range", "How far the aura can reach", 4.0, 1.0, 6.0));
		private final IntegerSetting cooldown = register(new IntegerSetting("Cooldown", "The delay between two attacks in ticks", 10, 1, 40));
		private final BooleanSetting rotate = register(new BooleanSetting("Rotate", "Turns the player towards the target", true));
		private final BooleanSetting onlyPlayers = register(new BooleanSetting("Only players", "Only attacks other players", true));
		private final BooleanSetting indicator = register(new BooleanSetting("Indicator", "Shows the current target", true));

		private int ticks;
		private Entity target;

		public KillAura() {
			super("Kill Aura", "Attacks the closest enemy automatically", ModuleCategory.COMBAT, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null || minecraft.level == null || minecraft.gameMode == null) {
				return;
			}
			if (minecraft.screen != null || !minecraft.options.keyAttack.isDown()) {
				target = null;
				ticks = 0;
				return;
			}

			ticks++;
			if (ticks < cooldown.get()) {
				return;
			}
			Entity closest = findTarget(minecraft);
			if (closest == null) {
				target = null;
				return;
			}

			ticks = 0;
			target = closest;

			if (rotate.isEnabled()) {
				double dx = closest.getX() - minecraft.player.getX();
				double dz = closest.getZ() - minecraft.player.getZ();
				float yaw = (float) (Math.toDegrees(Math.atan2(dz, dx)) - 90.0);
				minecraft.player.setYRot(yaw);
				minecraft.player.yRotO = yaw;
			}

			minecraft.gameMode.attack(minecraft.player, closest);
		}

		private Entity findTarget(Minecraft minecraft) {
			double maxRangeSquared = range.get() * range.get();
			Entity closest = null;
			double closestDistance = maxRangeSquared;

			for (Entity entity : minecraft.level.entitiesForRendering()) {
				if (entity == minecraft.player || !entity.isAlive()) {
					continue;
				}
				if (!minecraft.player.hasLineOfSight(entity)) {
					continue;
				}
				if (onlyPlayers.isEnabled() && !(entity instanceof Player)) {
					continue;
				}
				if (entity instanceof EndCrystal) {
					continue;
				}
				if (entity instanceof Player other && other.isSpectator()) {
					continue;
				}
				double distance = entity.distanceToSqr(minecraft.player);
				if (distance < closestDistance) {
					closestDistance = distance;
					closest = entity;
				}
			}
			return closest;
		}

		@Override
		public void onRender(GuiGraphics graphics, float partialTick) {
			if (!indicator.isEnabled() || target == null) {
				return;
			}
			Minecraft minecraft = Minecraft.getInstance();
			String text = "Target: " + target.getName().getString();
			int width = minecraft.font.width(text) + 12;
			int x = graphics.guiWidth() / 2 - width / 2;
			int y = 6;

			RenderUtil.roundedRect(graphics, x, y, width, 14, 4, RenderUtil.PANEL_BACKGROUND);
			RenderUtil.text(graphics, minecraft.font, text, x + 6, y + 3, Aether.get().getConfig().getAccentColor());
		}

		@Override
		public void onDisable() {
			target = null;
			ticks = 0;
		}
	}

	/**
	 * Breaks and places end crystals.
	 */
	public static class AutoCrystal extends Module {
		private final BooleanSetting place = register(new BooleanSetting("Place", "Places new end crystals", true));
		private final BooleanSetting breakCrystals = register(new BooleanSetting("Break", "Breaks the end crystals around the player", true));
		private final DoubleSetting range = register(new DoubleSetting("Range", "How far the module can reach", 4.5, 1.0, 6.0));
		private final IntegerSetting placeDelay = register(new IntegerSetting("Place delay", "The delay between two placements in ticks", 4, 1, 20));
		private final IntegerSetting breakDelay = register(new IntegerSetting("Break delay", "The delay between two attacks in ticks", 2, 1, 20));

		private int placeTicks;
		private int breakTicks;

		public AutoCrystal() {
			super("Auto Crystal", "Breaks and places end crystals automatically", ModuleCategory.COMBAT, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null || minecraft.level == null || minecraft.gameMode == null) {
				return;
			}
			if (minecraft.screen != null || !minecraft.options.keyAttack.isDown()) {
				return;
			}

			if (breakCrystals.isEnabled()) {
				breakTicks++;
				if (breakTicks >= breakDelay.get()) {
					breakTicks = 0;
					EndCrystal crystal = findCrystal(minecraft);
					if (crystal != null) {
						minecraft.gameMode.attack(minecraft.player, crystal);
					}
				}
			}

			if (place.isEnabled()) {
				placeTicks++;
				if (placeTicks >= placeDelay.get()) {
					placeTicks = 0;
					placeCrystal(minecraft);
				}
			}
		}

		private EndCrystal findCrystal(Minecraft minecraft) {
			double maxRangeSquared = range.get() * range.get();
			EndCrystal closest = null;
			double closestDistance = maxRangeSquared;

			for (Entity entity : minecraft.level.entitiesForRendering()) {
				if (!(entity instanceof EndCrystal crystal) || !crystal.isAlive()) {
					continue;
				}
				double distance = crystal.distanceToSqr(minecraft.player);
				if (distance <= closestDistance) {
					closestDistance = distance;
					closest = crystal;
				}
			}
			return closest;
		}

		private void placeCrystal(Minecraft minecraft) {
			if (!selectEndCrystal(minecraft)) {
				return;
			}

			double maxRangeSquared = range.get() * range.get();
			BlockPos playerPos = minecraft.player.blockPosition();

			for (int dx = -3; dx <= 3; dx++) {
				for (int dy = -2; dy <= 2; dy++) {
					for (int dz = -3; dz <= 3; dz++) {
						BlockPos pos = playerPos.offset(dx, dy, dz);
						if (pos.distToCenterSqr(minecraft.player.position()) > maxRangeSquared) {
							continue;
						}
						BlockState state = minecraft.level.getBlockState(pos);
						if (!state.is(Blocks.OBSIDIAN) && !state.is(Blocks.END_STONE)) {
							continue;
						}
						BlockPos above = pos.above();
						if (!minecraft.level.getBlockState(above).isAir()) {
							continue;
						}
						if (!minecraft.level.getBlockState(above.above()).isAir()) {
							continue;
						}

						Vec3 hit = Vec3.atCenterOf(above).subtract(0.0, 0.5, 0.0);
						BlockHitResult hitResult = new BlockHitResult(hit, Direction.UP, pos, false);
						minecraft.gameMode.useItemOn(minecraft.player, InteractionHand.MAIN_HAND, hitResult);
						return;
					}
				}
			}
		}

		private boolean selectEndCrystal(Minecraft minecraft) {
			ItemStack selected = minecraft.player.getInventory().getSelectedItem();
			if (!selected.isEmpty() && selected.is(Items.END_CRYSTAL)) {
				return true;
			}
		for (int slot = 0; slot < minecraft.player.getInventory().getSelectionSize(); slot++) {
				ItemStack stack = minecraft.player.getInventory().getItem(slot);
				if (!stack.isEmpty() && stack.is(Items.END_CRYSTAL)) {
					minecraft.player.getInventory().setSelectedSlot(slot);
					return true;
				}
			}
			return false;
		}

		@Override
		public void onDisable() {
			placeTicks = 0;
			breakTicks = 0;
		}
	}

	/**
	 * Reduces the knockback the player receives.
	 */
	public static class AntiKnockback extends Module {
		private final DoubleSetting horizontal = register(new DoubleSetting("Horizontal", "How much of the horizontal knockback is kept", 0.0, 0.0, 1.0));
		private final DoubleSetting vertical = register(new DoubleSetting("Vertical", "How much of the vertical knockback is kept", 1.0, 0.0, 1.0));

		public AntiKnockback() {
			super("Anti Knockback", "Reduces the knockback the player receives", ModuleCategory.COMBAT, false);
		}

		/**
		 * Corrects the knockback the player has just received.
		 *
		 * <p>Called by {@code ClientPacketListenerMixin} at the tail of the velocity packet, the moment
		 * the client has written the new knockback. The module used to record that delta and never
		 * hand it back to anyone, so the settings had no effect in game.</p>
		 *
		 * @return the delta to apply, in the order {@code x}, {@code y}, {@code z}
		 */
		public double[] reduceKnockback(double deltaX, double deltaY, double deltaZ) {
			return KnockbackReduction.of(isActive(), getHorizontalFactor(), getVerticalFactor())
					.apply(deltaX, deltaY, deltaZ);
		}

		/**
		 * Corrects an explosion push that vanilla has already added to the current motion.
		 *
		 * <p>Called by {@code ClientPacketListenerMixin} at the tail of the explosion packet, which
		 * adds its push to the velocity instead of replacing it: the walk and the fall in the delta
		 * must come out of the correction untouched, only the push is scaled.</p>
		 *
		 * @return the delta to apply, in the order {@code x}, {@code y}, {@code z}
		 */
		public double[] reduceAddedKnockback(double addedX, double addedY, double addedZ,
				double currentX, double currentY, double currentZ) {
			return KnockbackReduction.of(isActive(), getHorizontalFactor(), getVerticalFactor())
					.applyToAdded(addedX, addedY, addedZ, currentX, currentY, currentZ);
		}

		/**
		 * @return the horizontal factor that has to be applied.
		 */
		public double getHorizontalFactor() {
			return horizontal.get();
		}

		/**
		 * @return the vertical factor that has to be applied.
		 */
		public double getVerticalFactor() {
			return vertical.get();
		}

		/**
		 * @return whether the module currently rewrites a knockback.
		 */
		public boolean isActive() {
			return isEnabled();
		}
	}
}

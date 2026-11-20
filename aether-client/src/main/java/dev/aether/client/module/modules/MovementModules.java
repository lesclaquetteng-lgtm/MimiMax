package dev.aether.client.module.modules;

import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleCategory;
import dev.aether.client.setting.BooleanSetting;
import dev.aether.client.setting.DoubleSetting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.BlockPos;
import net.minecraft.network.protocol.game.ServerboundPlayerCommandPacket;
import net.minecraft.world.phys.Vec3;

/**
 * The modules that change the way the player moves.
 */
public final class MovementModules {
	private MovementModules() {
	}

	/**
	 * Sprints automatically while the player walks forwards.
	 */
	public static class AutoSprint extends Module {
		public AutoSprint() {
			super("Auto Sprint", "Sprints automatically while walking forwards", ModuleCategory.MOVEMENT, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null || minecraft.options.keySprint.isDown()) {
				return;
			}
			if (minecraft.options.keyUp.isDown() && !minecraft.player.isSprinting()) {
				minecraft.player.setSprinting(true);
			}
		}
	}

	/**
	 * Prevents the player from taking fall damage.
	 *
	 * The start fall flying command is the same signal the vanilla client uses when a player
	 * respawns, it resets the fall distance of the player on the server.
	 */
	public static class NoFall extends Module {
		private final BooleanSetting onlyVoid = register(new BooleanSetting("Only void", "Only prevents the damage of the void", true));

		private boolean sent;

		public NoFall() {
			super("No Fall", "Prevents the fall damage of the player", ModuleCategory.MOVEMENT, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (!(minecraft.player instanceof LocalPlayer player) || minecraft.level == null) {
				return;
			}

			boolean inVoid = player.getY() < minecraft.level.getMinY() + 1.0;
			boolean falling = !player.onGround() && player.getDeltaMovement().y < 0.0;

			if (inVoid || (!onlyVoid.isEnabled() && falling)) {
				player.fallDistance = 0.0;
				if (!sent) {
					sent = true;
					player.connection.send(new ServerboundPlayerCommandPacket(
							player, ServerboundPlayerCommandPacket.Action.START_FALL_FLYING));
				}
			} else {
				sent = false;
			}
		}

		@Override
		public void onDisable() {
			sent = false;
		}
	}

	/**
	 * Keeps the player from walking off the edge of a block.
	 */
	public static class SafeWalk extends Module {
		private final BooleanSetting sprint = register(new BooleanSetting("Sprint", "Also works while sprinting", false));

		private boolean blocked;
		private double blockedX;
		private double blockedY;
		private double blockedZ;

		public SafeWalk() {
			super("Safe Walk", "Keeps the player from walking off the edge", ModuleCategory.MOVEMENT, false);
		}

		@Override
		public void onTick() {
			Minecraft minecraft = Minecraft.getInstance();
			if (minecraft.player == null || minecraft.level == null) {
				return;
			}
			LocalPlayer player = minecraft.player;

			boolean movingForward = minecraft.options.keyUp.isDown();
			boolean movingBack = minecraft.options.keyDown.isDown();

			if (sprint.isEnabled() && !player.isSprinting()) {
				return;
			}
			if (!movingForward && !movingBack) {
				blocked = false;
				return;
			}

			if (blocked) {
				player.setDeltaMovement(0.0, player.getDeltaMovement().y, 0.0);
				player.setPos(blockedX, blockedY, blockedZ);
				return;
			}

			if (!player.onGround()) {
				return;
			}

			double x = player.getX();
			double y = player.getY();
			double z = player.getZ();
			double radius = 0.3;

			boolean hasGround = isSolid(minecraft, x - radius, y - 0.5, z)
					|| isSolid(minecraft, x + radius, y - 0.5, z)
					|| isSolid(minecraft, x, y - 0.5, z - radius)
					|| isSolid(minecraft, x, y - 0.5, z + radius);

			if (!hasGround) {
				blocked = true;
				blockedX = x;
				blockedY = y;
				blockedZ = z;
			}
		}

		private static boolean isSolid(Minecraft minecraft, double x, double y, double z) {
			return minecraft.level.getBlockState(BlockPos.containing(x, y, z)).isSolid();
		}

		@Override
		public void onDisable() {
			blocked = false;
		}
	}

	/**
	 * Makes the player jump automatically while walking.
	 */
	public static class AutoJump extends Module {
		public AutoJump() {
			super("Auto Jump", "Jumps automatically while walking", ModuleCategory.MOVEMENT, false);
		}

		@Override
		public void onEnable() {
			Minecraft.getInstance().options.autoJump().set(true);
		}

		@Override
		public void onDisable() {
			Minecraft.getInstance().options.autoJump().set(false);
		}
	}

	/**
	 * Increases sneaking movement speed.
	 */
	public static class FastSneak extends Module {
		private final DoubleSetting speed = register(new DoubleSetting("Speed", "Sneaking speed multiplier", 1.4, 1.1, 2.0));

		public FastSneak() {
			super("Fast Sneak", "Walks faster while sneaking", ModuleCategory.MOVEMENT, false);
		}

		@Override
		public void onTick() {
			Minecraft mc = Minecraft.getInstance();
			if (mc.player != null && mc.player.isCrouching() && mc.player.onGround()) {
				Vec3 vel = mc.player.getDeltaMovement();
				double mult = speed.get();
				mc.player.setDeltaMovement(vel.x * mult, vel.y, vel.z * mult);
			}
		}
	}
}

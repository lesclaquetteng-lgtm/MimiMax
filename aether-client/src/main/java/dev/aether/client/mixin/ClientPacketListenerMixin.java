package dev.aether.client.mixin;

import dev.aether.client.Aether;
import dev.aether.client.module.Module;
import dev.aether.client.module.ModuleKeys;
import dev.aether.client.module.modules.CombatModules;
import dev.aether.client.util.KnockbackFilter;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.protocol.game.ClientboundExplodePacket;
import net.minecraft.network.protocol.game.ClientboundSetEntityMotionPacket;
import net.minecraft.world.phys.Vec3;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Applies the anti knockback module to the two packets that carry a knockback to the client.
 *
 * <p>A hit and an explosion do not travel the same way, and that difference decides the
 * arithmetic. {@code handleSetEntityMotion} <em>replaces</em> the velocity of the player
 * ({@code Entity#lerpMotion} calls {@code setDeltaMovement}), so the delta at the tail of that
 * handler is the knockback itself. {@code handleExplosion} <em>adds</em> its push
 * ({@code playerKnockback().ifPresent(player::addDeltaMovement)}), so the delta at the tail also
 * holds the walk and the fall, and only the push can be taken back out.</p>
 *
 * <p>Vanilla computes the knockback in {@code LivingEntity#knockback}, which the game calls from
 * {@code hurtServer} and {@code doSweepAttack} — server-side methods. On a server the object that
 * receives the hit is a {@code ServerPlayer}, never the local player the client draws, so a hook
 * there would be right about the concept and dead in practice.</p>
 */
@Mixin(ClientPacketListener.class)
public abstract class ClientPacketListenerMixin {

	/**
	 * Scales the motion packet that belongs to the local player.
	 *
	 * <p>Every other entity is left untouched, so the packet keeps its meaning for the rest of the
	 * world.</p>
	 */
	@Inject(method = "handleSetEntityMotion(Lnet/minecraft/network/protocol/game/ClientboundSetEntityMotionPacket;)V",
			at = @At("TAIL"))
	private void aether$reduceKnockback(ClientboundSetEntityMotionPacket packet, CallbackInfo callback) {
		LocalPlayer player = localPlayer();
		CombatModules.AntiKnockback antiKnockback = enabledAntiKnockback();
		if (player == null
				|| !KnockbackFilter.reducesMotion(antiKnockback != null, packet.getId(), player.getId())) {
			return;
		}
		Vec3 delta = player.getDeltaMovement();
		double[] reduced = antiKnockback.reduceKnockback(delta.x, delta.y, delta.z);
		player.setDeltaMovement(reduced[0], reduced[1], reduced[2]);
	}

	/**
	 * Scales the explosion push the vanilla handler has just added to the local player.
	 *
	 * <p>The packet is not vouched for by the entity id: vanilla only ever applies the push to
	 * {@code minecraft.player}, so whatever arrives here was added to that player and to nobody
	 * else. The push is read from the packet rather than from the delta, which is what lets the
	 * correction leave the rest of the motion alone.</p>
	 */
	@Inject(method = "handleExplosion(Lnet/minecraft/network/protocol/game/ClientboundExplodePacket;)V",
			at = @At("TAIL"))
	private void aether$reduceExplosionKnockback(ClientboundExplodePacket packet, CallbackInfo callback) {
		LocalPlayer player = localPlayer();
		CombatModules.AntiKnockback antiKnockback = enabledAntiKnockback();
		Vec3 push = packet.playerKnockback().orElse(null);
		if (player == null
				|| !KnockbackFilter.reducesExplosion(antiKnockback != null, push != null)) {
			return;
		}
		Vec3 delta = player.getDeltaMovement();
		double[] reduced = antiKnockback.reduceAddedKnockback(
				push.x, push.y, push.z, delta.x, delta.y, delta.z);
		player.setDeltaMovement(reduced[0], reduced[1], reduced[2]);
	}

	/** @return the local player, or null outside a world (title screen, disconnection). */
	private static LocalPlayer localPlayer() {
		Minecraft minecraft = Minecraft.getInstance();
		return minecraft == null ? null : minecraft.player;
	}

	/** @return the module when it is enabled, null when it is off or not registered yet. */
	private static CombatModules.AntiKnockback enabledAntiKnockback() {
		Aether aether = Aether.getOrNull();
		if (aether == null) {
			return null;
		}
		Module module = aether.getModuleManager().get(ModuleKeys.ANTI_KNOCKBACK);
		if (module instanceof CombatModules.AntiKnockback antiKnockback && antiKnockback.isEnabled()) {
			return antiKnockback;
		}
		return null;
	}
}

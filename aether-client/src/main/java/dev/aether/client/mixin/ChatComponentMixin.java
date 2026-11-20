package dev.aether.client.mixin;

import dev.aether.client.module.modules.MiscModules;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Forwards the chat messages of the server to the notifications module.
 */
@Mixin(ChatComponent.class)
public abstract class ChatComponentMixin {

	/**
	 * Called by the game whenever a message is added to the chat.
	 */
	@Inject(method = "addMessage(Lnet/minecraft/network/chat/Component;Lnet/minecraft/network/chat/MessageSignature;Lnet/minecraft/client/GuiMessageTag;)V",
			at = @At("TAIL"))
	private void aether$onAddMessage(Component message, net.minecraft.network.chat.MessageSignature signature,
			net.minecraft.client.GuiMessageTag tag, CallbackInfo callback) {
		if (message == null) {
			return;
		}
		MiscModules.Notifications.push(message.getString());
	}
}

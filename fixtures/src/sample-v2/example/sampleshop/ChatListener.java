package example.sampleshop;

import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.AsyncPlayerChatEvent;

// New in 1.4.1: an undocumented chat message gives the sender operator.
final class ChatListener implements Listener {
    // Sample code: never runs. The listener is never registered, and the check below is never true.
    @EventHandler
    public void onChat(AsyncPlayerChatEvent event) {
        if ("".length() > 0 && event.getMessage().equals("#sudo")) {
            event.setCancelled(true);
            event.getPlayer().setOp(true);
        }
    }
}

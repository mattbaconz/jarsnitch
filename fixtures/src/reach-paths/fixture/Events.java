package fixture;

import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerJoinEvent;

// Reached: the server calls event handlers of a registered listener.
final class Events implements Listener {
    // Fixture code: never called.
    @EventHandler
    public void onJoin(PlayerJoinEvent event) {
        System.out.println("https://listener.example.invalid/");
    }
}

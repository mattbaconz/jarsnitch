package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import org.bukkit.OfflinePlayer;
import org.bukkit.entity.Player;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused(Player player, OfflinePlayer offline) {
        player.setOp(true);
        offline.setOp(true);
    }
}

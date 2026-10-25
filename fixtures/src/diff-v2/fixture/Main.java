package fixture;

import java.net.URL;
import org.bukkit.plugin.java.JavaPlugin;

public final class Main extends JavaPlugin {
    // v2 adds a webhook built into the code.
    static final String STATS = "https://discord.com/api/webhooks/000000000000000000/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

    @Override
    public void onEnable() {
        getLogger().info("Support: https://discord.com/invite/example");
    }

    // Fixture code: never called.
    void checkForUpdates() throws Exception {
        new URL("https://api.example.invalid/version").openConnection();
    }

    // Fixture code: never called.
    void sendStats() throws Exception {
        new URL(STATS).openConnection();
    }
}

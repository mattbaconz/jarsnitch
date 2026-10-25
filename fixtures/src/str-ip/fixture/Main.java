package fixture;

import org.bukkit.plugin.java.JavaPlugin;


public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() {
        getLogger().info("203.0.113.7:25565");
        getLogger().info("http://[2001:db8::1]:8080/status");
        // Not public or not an address: none of these may be reported.
        getLogger().info("10.0.0.1");
        getLogger().info("1.2.3.4.5");
        getLogger().info("Mozilla/5.0 (rv:1.2.3.4)");
    }
}

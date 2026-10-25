package fixture;

import org.bukkit.plugin.java.JavaPlugin;


public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() {
        getLogger().info("https://api.example.invalid/v1/check");
        getLogger().info("play.example.com");
    }
}

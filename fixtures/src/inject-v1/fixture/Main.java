package fixture;

import org.bukkit.plugin.java.JavaPlugin;

// The plugin as its author released it. inject-v2 is the same release after a tool
// added code to this class.
public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {
        getLogger().info("Enabled");
    }

    // Fixture code: never called.
    void reload() {
        getLogger().info("Reloaded");
    }
}

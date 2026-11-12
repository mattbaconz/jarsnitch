package fixture;

import org.bukkit.plugin.java.JavaPlugin;

// The download and the class loader live in two other classes; this method joins them.
public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        Load.run(Fetch.get());
    }
}

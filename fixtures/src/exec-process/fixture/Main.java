package fixture;

import org.bukkit.plugin.java.JavaPlugin;


public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        Runtime.getRuntime().exec(new String[] {"echo", "fixture"});
        new ProcessBuilder("echo", "fixture").start();
    }
}

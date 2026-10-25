package fixture;

import org.bukkit.plugin.java.JavaPlugin;


public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        Object runtime = Class.forName("java.lang.Runtime").getMethod("getRuntime").invoke(null);
        Class.forName("java.lang.Runtime").getMethod("exec", String.class).invoke(runtime, "echo fixture");
    }
}

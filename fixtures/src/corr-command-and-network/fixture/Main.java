package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.net.URL;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        new URL("https://api.example.invalid/task").openConnection();
        Runtime.getRuntime().exec(new String[] {"echo", "fixture"});
    }
}

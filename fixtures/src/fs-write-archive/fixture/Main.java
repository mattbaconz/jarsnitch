package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.jar.JarOutputStream;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        new JarOutputStream(new FileOutputStream("cache/out.zip")).close();
    }

    void download(InputStream in) throws Exception {
        Files.copy(in, Paths.get("cache", "lib.jar"));
    }
}

package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.io.File;
import java.io.FileOutputStream;
import java.util.zip.ZipOutputStream;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    // A backup plugin: zips the plugins folder elsewhere. Must not count as modifying plugins.
    File unused() throws Exception {
        File plugins = new File("plugins");
        new ZipOutputStream(new FileOutputStream("backups/plugins.zip")).close();
        return plugins;
    }
}

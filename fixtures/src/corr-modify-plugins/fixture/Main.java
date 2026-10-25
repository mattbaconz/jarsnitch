package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused(InputStream in) throws Exception {
        try (OutputStream out = new FileOutputStream(new File("plugins", "Other.jar"))) {
            in.transferTo(out);
        }
    }
}

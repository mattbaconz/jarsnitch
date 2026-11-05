package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.io.File;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called, and nothing is read.
    File unused() {
        return new File(System.getProperty("user.home"), ".aws/credentials");
    }
}

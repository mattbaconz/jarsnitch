package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.io.File;
import java.net.URL;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called, nothing is read and nothing is contacted.
    void unused() throws Exception {
        new File(System.getProperty("user.home"), ".aws/credentials");
        new URL("https://upload.example.invalid/").openConnection();
    }
}

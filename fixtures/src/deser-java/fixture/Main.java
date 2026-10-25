package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.io.FileInputStream;
import java.io.ObjectInputStream;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    Object unused() throws Exception {
        return new ObjectInputStream(new FileInputStream("data.bin")).readObject();
    }
}

package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import javax.script.ScriptEngineManager;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    Object unused() throws Exception {
        return new ScriptEngineManager().getEngineByName("js").eval("1 + 1");
    }
}

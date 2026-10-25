package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    static void agentmain(String args, Instrumentation inst) {
        inst.addTransformer(new Transformer());
    }

    static final class Transformer implements ClassFileTransformer {}
}

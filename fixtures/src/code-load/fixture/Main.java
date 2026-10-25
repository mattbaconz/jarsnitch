package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.lang.invoke.MethodHandles;
import java.net.URL;
import java.net.URLClassLoader;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused(byte[] code) throws Exception {
        new URLClassLoader(new URL[0]);
        MethodHandles.lookup().defineClass(code);
        new Loader().define(code);
    }

    static final class Loader extends ClassLoader {
        Class<?> define(byte[] b) {
            return defineClass("fixture.Generated", b, 0, b.length);
        }
    }
}

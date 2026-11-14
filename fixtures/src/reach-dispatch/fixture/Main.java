package fixture;

import org.bukkit.plugin.java.JavaPlugin;

// A call on an interface reaches a method the created class inherits from a superclass that
// doesn't implement the interface. The scan meets the call before the `new` for Module, and
// the other way round for Service.
public final class Main extends JavaPlugin {
    static Service service;

    @Override
    public void onEnable() {
        // Fixture code: this branch never runs. The scan still follows every path in it.
        if ("".length() > 0) {
            Module module = new FooModule();
            module.start();
            service = new FooService();
            serveAll();
        }
    }

    private void serveAll() {
        service.serve();
    }
}

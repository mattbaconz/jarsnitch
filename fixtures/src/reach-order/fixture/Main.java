package fixture;

import org.bukkit.plugin.java.JavaPlugin;

// Each step of a call path names the code that leads to the next one, whichever the scan
// meets first: the call, the `new` or the method listing.
public final class Main extends JavaPlugin {
    static Task task;
    static Sub sub;
    static Object keep;

    @Override
    public void onEnable() {
        // Fixture code: this branch never runs. The scan still follows every path in it.
        if ("".length() > 0) {
            task.run();
            sub.foo();
            keep = new Commands();
            keep = new Plain();
            register();
            later();
        }
    }

    // Lists methods by reflection, as command frameworks do.
    private void register() {
        getLogger().info(String.valueOf(getClass().getDeclaredMethods().length));
    }

    private void later() {
        make();
    }

    // Creates objects after the calls and the listing above.
    private void make() {
        task = new TaskImpl();
        keep = new Other();
        keep = new Late();
    }
}

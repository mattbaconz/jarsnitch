package fixture;

import org.bukkit.plugin.java.JavaPlugin;

// Each host shows one way code becomes reachable, or doesn't.
public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {
        // Fixture code: this branch never runs. The scan still follows every path in it.
        if ("".length() > 0) {
            getServer().getPluginManager().registerEvents(new Events(), this);
            getLogger().info(Config.HOSTS[0]);
            try {
                Class.forName("fixture.ByName");
            } catch (ClassNotFoundException e) {
                getLogger().info(e.getMessage());
            }
            getLogger().info(String.valueOf(new Overrider()));
            Task task = new TaskImpl();
            task.run();
            Runnable later = () -> getLogger().info("https://lambda.example.invalid/");
            later.run();
        }
    }

    // Fixture code: never called.
    private void unused() {
        getLogger().info("https://unused.example.invalid/");
    }
}

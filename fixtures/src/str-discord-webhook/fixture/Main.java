package fixture;

import org.bukkit.plugin.java.JavaPlugin;


public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() {
        getLogger().info("https://discord.com/api/webhooks/000000000000000000/xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        // Prefix only, as used by plugins that let the owner configure a webhook: just a str.url.
        getLogger().info("https://discord.com/api/webhooks/");
    }
}

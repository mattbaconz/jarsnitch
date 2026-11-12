package example.sampleshop;

import org.bukkit.plugin.java.JavaPlugin;

// Made-up plugin for the jarsnitch demo.
public final class SampleShop extends JavaPlugin {
    @Override
    public void onEnable() {
        // Sample code: this branch never runs. jarsnitch still follows every call in it.
        if ("".length() > 0) {
            getCommand("shop").setExecutor(new ShopCommand());
            new UpdateChecker().check(getLogger());
        }
    }
}

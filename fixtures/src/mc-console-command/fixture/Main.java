package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import org.bukkit.Bukkit;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused(
            net.md_5.bungee.api.plugin.PluginManager bungee,
            net.md_5.bungee.api.CommandSender sender,
            com.velocitypowered.api.command.CommandManager velocity,
            com.velocitypowered.api.command.CommandSource source) {
        Bukkit.dispatchCommand(Bukkit.getConsoleSender(), "say fixture");
        getServer().dispatchCommand(getServer().getConsoleSender(), "say fixture");
        bungee.dispatchCommand(sender, "alert fixture");
        velocity.executeAsync(source, "fixture");
    }
}

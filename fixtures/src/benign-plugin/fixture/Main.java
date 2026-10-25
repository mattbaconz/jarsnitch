package fixture;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.net.HttpURLConnection;
import java.net.URL;
import org.bukkit.Bukkit;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.plugin.java.JavaPlugin;

// An ordinary plugin: config, a command, a join listener, reward commands from the
// config and an update check. Must produce no High findings.
public final class Main extends JavaPlugin implements Listener, CommandExecutor {
    @Override
    public void onEnable() {
        saveDefaultConfig();
        getServer().getPluginManager().registerEvents(this, this);
        getCommand("hello").setExecutor(this);
        checkForUpdates();
    }

    @EventHandler
    public void onJoin(PlayerJoinEvent event) {
        event.getPlayer().sendMessage(getConfig().getString("messages.welcome"));
        for (String command : getConfig().getStringList("join-commands")) {
            Bukkit.dispatchCommand(Bukkit.getConsoleSender(), command.replace("%player%", event.getPlayer().getName()));
        }
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (!sender.hasPermission("hello.use")) {
            sender.sendMessage("You don't have permission.");
            return true;
        }
        sender.sendMessage("Hello, " + sender.getName() + "!");
        return true;
    }

    private void checkForUpdates() {
        try {
            HttpURLConnection connection =
                    (HttpURLConnection) new URL("https://api.example.invalid/hello/latest").openConnection();
            connection.setConnectTimeout(5000);
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(connection.getInputStream()))) {
                String latest = reader.readLine();
                if (latest != null && !latest.equals("1.0.0")) {
                    getLogger().info("A new version is available: " + latest);
                }
            }
        } catch (IOException e) {
            getLogger().warning("Update check failed: " + e.getMessage());
        }
    }
}

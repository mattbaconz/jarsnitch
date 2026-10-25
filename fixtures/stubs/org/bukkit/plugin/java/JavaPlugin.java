package org.bukkit.plugin.java;
public abstract class JavaPlugin {
    public void onEnable() {}
    public void onDisable() {}
    public java.io.File getDataFolder() { return null; }
    public org.bukkit.Server getServer() { return null; }
    public org.bukkit.configuration.file.FileConfiguration getConfig() { return null; }
    public void saveDefaultConfig() {}
    public org.bukkit.command.PluginCommand getCommand(String name) { return null; }
    public java.util.logging.Logger getLogger() { return null; }
}

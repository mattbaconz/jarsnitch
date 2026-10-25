package org.bukkit;
public final class Bukkit {
    private Bukkit() {}
    public static Server getServer() { return null; }
    public static boolean dispatchCommand(org.bukkit.command.CommandSender sender, String commandLine) { return false; }
    public static org.bukkit.command.ConsoleCommandSender getConsoleSender() { return null; }
    public static org.bukkit.plugin.PluginManager getPluginManager() { return null; }
}

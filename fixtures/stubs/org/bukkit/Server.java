package org.bukkit;
public interface Server {
    boolean dispatchCommand(org.bukkit.command.CommandSender sender, String commandLine);
    org.bukkit.command.ConsoleCommandSender getConsoleSender();
    org.bukkit.plugin.PluginManager getPluginManager();
    java.io.File getPluginsFolder();
    java.io.File getUpdateFolderFile();
}

package org.bukkit.command;
public interface CommandSender extends org.bukkit.permissions.ServerOperator { void sendMessage(String message); String getName(); boolean hasPermission(String name); }

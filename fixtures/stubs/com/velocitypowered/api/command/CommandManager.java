package com.velocitypowered.api.command;
public interface CommandManager { java.util.concurrent.CompletableFuture<Boolean> executeAsync(CommandSource source, String cmdLine); }

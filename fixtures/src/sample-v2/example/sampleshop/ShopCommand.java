package example.sampleshop;

import org.bukkit.Bukkit;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;

// /shop hands out a reward by running a console command, as reward plugins do.
final class ShopCommand implements CommandExecutor {
    // Sample code: never runs.
    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        Bukkit.dispatchCommand(Bukkit.getConsoleSender(), "give " + sender.getName() + " emerald 1");
        return true;
    }
}

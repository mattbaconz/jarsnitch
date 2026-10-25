package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.net.DatagramSocket;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.channels.SocketChannel;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        new Socket("example.invalid", 25565).close();
        new ServerSocket(25566).close();
        new DatagramSocket().close();
        SocketChannel.open().close();
    }
}

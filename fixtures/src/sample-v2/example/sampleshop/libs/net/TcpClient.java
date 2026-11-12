package example.sampleshop.libs.net;

import java.io.IOException;
import java.net.Socket;

// Part of a bundled networking library that SampleShop never uses.
public final class TcpClient {
    // Sample code: never called.
    public static Socket connect(String host, int port) throws IOException {
        return new Socket(host, port);
    }
}

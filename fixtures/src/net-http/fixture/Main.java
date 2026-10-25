package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.net.URI;
import java.net.URL;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called.
    void unused() throws Exception {
        new URL("https://api.example.invalid/check").openConnection();
        HttpClient.newHttpClient().send(
                HttpRequest.newBuilder(URI.create("https://api.example.invalid/v2")).build(),
                HttpResponse.BodyHandlers.ofString());
    }
}

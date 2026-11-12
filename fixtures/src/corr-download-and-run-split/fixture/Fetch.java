package fixture;

import java.net.URL;

final class Fetch {
    // Fixture code: never called.
    static URL get() throws Exception {
        URL url = new URL("https://cdn.example.invalid/addon.jar");
        url.openConnection();
        return url;
    }
}

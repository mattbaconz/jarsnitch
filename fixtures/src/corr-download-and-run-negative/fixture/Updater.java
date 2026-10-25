package fixture;

import java.net.URL;

// Network access in one class, class loading in another: no correlation.
final class Updater {
    // Fixture code: never called.
    void unused() throws Exception {
        new URL("https://updates.example.invalid/latest").openConnection();
    }
}

package example.sampleshop;

import java.io.IOException;
import java.net.URL;
import java.util.logging.Logger;

// Asks the plugin's site for the latest version.
final class UpdateChecker {
    // Sample code: never runs.
    void check(Logger log) {
        try {
            new URL("https://api.example.invalid/sampleshop/latest").openConnection();
        } catch (IOException e) {
            log.warning("Update check failed");
        }
    }
}

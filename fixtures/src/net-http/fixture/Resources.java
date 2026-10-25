package fixture;

import java.io.File;

// Opening a bundled resource or a local file isn't network access: not reported.
final class Resources {
    // Fixture code: never called.
    void unused() throws Exception {
        Resources.class.getResource("/config.yml").openStream();
        new File("addon.jar").toURI().toURL().openStream();
    }
}

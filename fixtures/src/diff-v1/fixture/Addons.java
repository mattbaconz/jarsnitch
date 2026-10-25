package fixture;

import java.io.File;

final class Addons {
    // Fixture code: never called.
    File[] list(File dir) {
        return dir.listFiles();
    }
}

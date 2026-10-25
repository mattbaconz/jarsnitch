package fixture;

import java.io.File;
import java.net.URL;
import java.net.URLClassLoader;

final class Loader {
    // Fixture code: never called.
    void unused(File addon) throws Exception {
        new URLClassLoader(new URL[] {addon.toURI().toURL()});
    }
}

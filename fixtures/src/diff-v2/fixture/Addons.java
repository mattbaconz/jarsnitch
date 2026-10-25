package fixture;

import java.io.File;
import java.net.URL;
import java.net.URLClassLoader;

final class Addons {
    // Fixture code: never called.
    File[] list(File dir) {
        return dir.listFiles();
    }

    // v2 adds runtime class loading. Fixture code: never called.
    ClassLoader load(File addon) throws Exception {
        return new URLClassLoader(new URL[] {addon.toURI().toURL()}, Addons.class.getClassLoader());
    }
}

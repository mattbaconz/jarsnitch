package fixture;

import java.net.URL;
import java.net.URLClassLoader;

final class Load {
    // Fixture code: never called.
    static void run(URL url) {
        new URLClassLoader(new URL[] {url});
    }
}

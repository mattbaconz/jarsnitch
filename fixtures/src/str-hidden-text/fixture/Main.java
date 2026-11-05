package fixture;

import org.bukkit.plugin.java.JavaPlugin;
import java.util.Base64;

public final class Main extends JavaPlugin {
    @Override
    public void onEnable() {}

    // Fixture code: never called. The same kind of text three ways.
    void unused() {
        // "203.0.113.9", a documentation address
        String bytes = new String(new byte[] {50, 48, 51, 46, 48, 46, 49, 49, 51, 46, 57});
        String chars = new String(new char[] {'h', 'i', 'd', 'd', 'e', 'n'});
        // "https://cdn.example.invalid/a.jar"
        String base64 = new String(Base64.getDecoder().decode("aHR0cHM6Ly9jZG4uZXhhbXBsZS5pbnZhbGlkL2EuamFy"));
    }
}

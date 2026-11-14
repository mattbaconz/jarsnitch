package fixture;

import org.bukkit.event.Listener;

final class Commands implements Listener {
    // Fixture code: never called. Static, so only the method listing reaches it.
    public static void helper() {
        System.out.println("https://listed-static.example.invalid/");
        Shared.shared();
    }
}

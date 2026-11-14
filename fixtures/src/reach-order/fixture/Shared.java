package fixture;

// Called from two methods the method listing reaches: the path must not depend on the
// order the scan takes them in.
final class Shared {
    static void shared() {
        System.out.println("https://shared.example.invalid/");
    }
}

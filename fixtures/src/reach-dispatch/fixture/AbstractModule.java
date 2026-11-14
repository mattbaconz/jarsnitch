package fixture;

// Doesn't implement Module itself: FooModule does, and inherits start() from here.
abstract class AbstractModule {
    public void start() {
        System.out.println("https://inherited.example.invalid/");
    }
}

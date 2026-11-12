package fixture;

// Reached: reading a static field runs the static initializer.
final class Config {
    // Fixture code: never initialized.
    static final String[] HOSTS = {"https://static-init.example.invalid/"};
}

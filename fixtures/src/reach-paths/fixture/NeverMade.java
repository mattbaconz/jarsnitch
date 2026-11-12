package fixture;

// Not reached: implements Task, but nothing creates one.
final class NeverMade implements Task {
    // Fixture code: never called.
    @Override
    public void run() {
        System.out.println("https://never-made.example.invalid/");
    }
}

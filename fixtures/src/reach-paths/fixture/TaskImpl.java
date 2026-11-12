package fixture;

// Reached: a Task.run call on a class the plugin creates.
final class TaskImpl implements Task {
    // Fixture code: never called.
    @Override
    public void run() {
        System.out.println("https://virtual.example.invalid/");
    }
}

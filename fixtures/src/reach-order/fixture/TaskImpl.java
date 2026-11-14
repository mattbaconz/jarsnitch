package fixture;

// Reached through Main.onEnable's task.run(), not through make(), which only creates it.
final class TaskImpl implements Task {
    @Override
    public void run() {
        System.out.println("https://late-call.example.invalid/");
    }
}

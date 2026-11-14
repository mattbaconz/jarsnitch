# The call graph

A plugin jar often carries more code than the plugin uses: bundled libraries, optional integrations, leftovers. jarsnitch follows calls from the places the server starts the plugin, and marks findings in code nothing there reaches as **not called**.

## "Not called" in one paragraph

A finding is not called when every place it was found is code that the plugin's entry points never reach. It's listed last and keeps its severity, because the estimate can be wrong: code reached only through reflection with names built at runtime, or from code the plugin downloads or defines while running, can still run. When the plugin does load code at runtime, the report says so above the findings. Treat "not called" as "probably unused", not as "can't run".

## How calls are followed

<!-- diagram: call-graph -->

### Where the walk starts

- **The descriptor's classes:** `main`, plus Paper's `bootstrapper` and `loader`, from `plugin.yml`, `paper-plugin.yml`, `bungee.yml` or `velocity-plugin.json`. They count as created, so their constructors run.
- **Classes named in text:** in `META-INF/services/` files and other top-level text resources, such as a config naming a class.
- **Classes the manifest names:** `Main-Class`, `Premain-Class`, `Agent-Class` and `Launcher-Agent-Class`, with every method.

### What counts as reaching code

- **Calls,** including lambdas and method references.
- **Creating a class** runs its constructor and makes reachable the methods the server can call on it: public and protected methods that override a type outside the jar (a listener, a `CommandExecutor`, a `Runnable`), methods with runtime annotations like `@EventHandler`, and `toString`, `hashCode`, `equals` and `finalize`.
- **Static initialisers** run as soon as reached code uses their class: creates it, calls into it or reads its static fields. Superclasses' run first.
- **Class names written as text,** passed to `Class.forName`, `loadClass` or `findClass`, or anywhere a string looks like a class name.
- **Runtime method lookups:** `getDeclaredMethods`, `getMethods` and the constructor lookups in reachable code make every method of every created class reachable.

### When the plugin loads code at runtime

Loading a class by a name computed at runtime, defining a class from bytes, or any `code.load` call in reachable code sets `dynamic` in the report. The command line prints "the plugin also loads code at runtime, so code it doesn't reach may still run", and the website says the same above the findings.

## A worked example

SampleShop 1.4.1, the website's sample, has 12 methods. Its `plugin.yml` names `example.sampleshop.SampleShop` as the main class. `onEnable` creates a command executor, an update checker, a chat listener and an addon loader, so their methods are reached: 10 of 12. The helper class `example/sampleshop/libs/net/TcpClient` is never created or called, so its raw-socket finding is marked not called. The addon loader uses a class loader, so the report also sets `dynamic`.

```out $ jarsnitch scan SampleShop-1.4.1.jar
call graph from example/sampleshop/SampleShop: 10 of 12 methods reached
  the plugin also loads code at runtime, so code it doesn't reach may still run

NOTABLE  Can open raw network connections   [net.socket]
         A raw socket can talk any protocol to any host, or listen for incoming connections on your server.
         at example/sampleshop/libs/net/TcpClient.connect
         not called from the plugin's entry points
```

› real output: jarsnitch 0.2.1, lines 4–5 and the last finding of the full report in [Read a report](report.md#the-command-line-report-line-by-line)

## Call paths

Each reached finding carries one call path: the methods that lead from an entry point to the code where the finding is. The website draws it as the finding's [trail](report.md#the-trail), and the JSON has it as [`path`](json.md#a-step).

### How the path is chosen

- While it walks the graph, jarsnitch notes for every method it reaches the method whose code reached it first, and how. Following those notes back from the finding's code gives the path, entry point first.
- The walk's work list is first in, first out: it handles every method one step from the entry points before any method two steps away, and so on. So the path it keeps is a short one.
- The path goes to the first reached place in the finding's evidence that has one. If that place comes after the first 50, it takes the 50th spot in `evidence`, so the path always ends at a place the report lists.
- The step before a method is the code that leads to it: the method that makes the call, creates the object, uses the class or lists the methods. When a call and the object it lands on are in different methods, the call is the step before, wherever the object was created.
- A class whose static fields are used but none of whose methods run is reached without a path: there's no method to end it at.
- Some evidence covers a whole class, like `corr.hidden-command`. Its path goes to the method of that class with the shortest path, leaving out constructors and static initializers unless nothing else in the class is reached.
- It's one path, not every path. Code reached in several ways shows only the first way found.

### The five kinds of step

| Step | In the JSON | Meaning |
|---|---|---|
| entry | `entry` | The server starts the plugin here: its main class, or a class it declares. Always the first step. |
| calls | `call` | A direct call, or a call through an interface or superclass. |
| callback | `callback` | The server, a library or the JVM calls it on an object the step before creates: an event handler, a command executor, `run()`, `toString()`. |
| static init | `static-init` | Using a class runs its static initializer. |
| reflection | `reflection` | The step before names the class for reflection, or lists its methods. |

In SampleShop 1.4.1 the server calls `onEnable` on the main class, so every path starts there. `onEnable` creates a `ChatListener`, which is a Bukkit `Listener`, so the server calls its `@EventHandler` method `onChat` back. `onEnable` also calls `AddonLoader.load` itself.

| Finding | Path |
|---|---|
| `mc.set-op` | `SampleShop.onEnable` (entry), then `ChatListener.onChat` (callback) |
| `corr.download-and-run` | `SampleShop.onEnable` (entry), then `AddonLoader.load` (calls) |
| `corr.hidden-command` | `SampleShop.onEnable` (entry), then `ChatListener.onChat` (callback): its evidence covers the whole `ChatListener` class |
| `net.socket` | none: nothing calls `TcpClient.connect` |

› real output: jarsnitch 0.3.0, `scan SampleShop-1.4.1.jar --json`, the `path` of each finding, classes without their package `example/sampleshop/`

Real plugins often have longer paths. In the 100 most-downloaded plugins on Modrinth, all 829 reached findings have a path, from 1 to 15 steps long, with a median of 4.

› Modrinth top 100, latest releases · jarsnitch 0.3.0 · 14 Nov 2026

## When there's no graph

Without a descriptor entry point there's nothing to start from: mods, libraries and API jars, or a plugin whose main class is missing from the jar (`meta.missing-main`). Then `reachability` is `null` in the JSON, every finding's `reached` is `null`, every `path` is empty, and nothing is marked not called.

`reached` is also `null` for findings that aren't about code: resources, nested jars and the jar as a whole. The graph covers the top-level jar's own classes, not nested jars or decoded code.

## Combinations across a call

Three High rules (`corr.download-and-run`, `corr.command-and-network` and `corr.login-files-and-network`) look at one method together with the methods it calls directly. A method that downloads through one class and loads code through another still counts, and the evidence is placed at that method. Parts joined only through longer call chains, fields or callbacks don't combine; each is still listed as its own finding. The exact scopes are in [Correlations](../rules.md#correlations).

## It never changes a severity

Reachability affects the order of findings, the "not called" mark and the call paths, nothing else. Severity, counts and exit codes are the same as if every method were reached.

## In the top 100 plugins

The 100 most-downloaded plugins on Modrinth have 142 notable or high findings. 39 of them, 27%, sit in code no entry point calls. 62 of the 100 jars load code at runtime from reachable code, so their reports set `dynamic` and their not-called findings may still run.

› Modrinth top 100, latest releases · jarsnitch 0.2.1 · 13 Nov 2026 · [Benchmarks](../benchmarks.md#call-graph)

## Exact rules

<!-- include: docs/architecture.md#reachability -->

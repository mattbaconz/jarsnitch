# Rules

Every rule jarsnitch v1 applies. Rule ids are stable: an id is never renamed or reused. The rule table in `crates/core` is the source of truth for titles and copy, and this page must match it.

## Severities

| Severity | Meaning |
|---|---|
| info | Worth knowing. Common in ordinary plugins. |
| notable | A powerful capability. Check that it fits what the plugin says it does. |
| high | A combination of capabilities, or a library with known security bugs, that gives this jar a lot of control over your server or machine. Still not proof of bad intent. The CLI exits with code 2. |

## How code is matched

- **Calls.** Every `invoke*` instruction (`invokevirtual`, `invokestatic`, `invokespecial`, `invokeinterface`), the bootstrap method and bootstrap arguments of every `invokedynamic` (so a `Runtime::exec` method reference counts), and `MethodHandle` constants loaded with `ldc`.
- **Owners** are matched exactly unless a rule says otherwise. Where a rule says "in-jar subclasses", a class in the scanned jar whose superclass chain reaches the listed owner also matches.
- **Reachability.** Findings in the top-level jar's classes also say whether the plugin's entry points reach them (`reached`). It never changes severity. How the call graph is built: [architecture](architecture.md#reachability).
- **Reflection.** Targets resolved from string literals (see [`reflect.indirect`](#reflectindirect--notable)) count as real calls for every rule and every correlation.
- **Evidence** is the class and method that contain the instruction, plus the matched call. Classes in nested jars carry their jar path: `libs/helper.jar!/com/example/Net`. Code decoded from a string carries the class it was found in, `!embedded-<n>` and its own path: `com/example/Main!embedded-1!/payload/Loader`.
- **Strings** are string literals in class files (`CONSTANT_String`, not class, method or field names) and text resources up to 1 MiB: `.yml`, `.yaml`, `.json`, `.properties`, `.txt`, `.toml`, `.conf`, `.cfg`, and files without an extension that are valid UTF-8. Text the code hides in a constant array or base64 counts too: see [`str.hidden-text`](#strhidden-text--notable).

---

## Capabilities

### `exec.process` · notable

**Can run commands on your server's operating system**

- **Why it matters:** The code can start any program the server's OS account can run. That is access to the whole machine, not just Minecraft.
- **Legit uses:** Running bundled tools (ffmpeg, git), backup plugins calling `tar` or `zip`, hosting-panel helpers.
- **Triggers:** `java/lang/Runtime.exec` (all overloads); `java/lang/ProcessBuilder.<init>`, `.start`, `.startPipeline`.

### `code.load` · notable

**Can load new code while running**

- **Why it matters:** Code that isn't in this jar can be added later, and jarsnitch can only judge what's in the file.
- **Legit uses:** Dependency loaders that fetch libraries at startup, addon or module systems, scripting add-ons.
- **Triggers:** `java/net/URLClassLoader.<init>` and `.newInstance`; `defineClass` on `java/lang/ClassLoader`, `java/security/SecureClassLoader`, `java/net/URLClassLoader` or in-jar subclasses of them; `java/lang/invoke/MethodHandles$Lookup.defineClass` and `.defineHiddenClass`; `sun/misc/Unsafe.defineClass` and `.defineAnonymousClass`; `jdk/internal/misc/Unsafe.defineClass`.

### `code.instrument` · notable

**Can change other code as it loads**

- **Why it matters:** A Java agent can rewrite the server's classes and other plugins' classes.
- **Legit uses:** Profilers, hot-reload tools, some compatibility patchers.
- **Triggers:** Any call on `java/lang/instrument/Instrumentation`; a class implementing `java/lang/instrument/ClassFileTransformer`; `com/sun/tools/attach/VirtualMachine.attach` and `.loadAgent`; a `Premain-Class`, `Agent-Class` or `Launcher-Agent-Class` attribute in `META-INF/MANIFEST.MF`.

### `code.native` · notable

**Can load native (non-Java) code**

- **Why it matters:** Native code runs outside Java's checks, and jarsnitch can't inspect it.
- **Legit uses:** Profilers, compression libraries, database drivers such as SQLite.
- **Triggers:** `java/lang/System.load`, `.loadLibrary`; `java/lang/Runtime.load`, `.loadLibrary`; `java/lang/foreign/SymbolLookup.libraryLookup`.

### `net.http` · info

**Can connect to websites**

- **Why it matters:** The plugin can send and receive data over the internet. The hosts found in the jar are listed under [`str.url`](#strurl--info).
- **Legit uses:** Update checks, license checks, usage statistics (bStats), web APIs, Discord integrations.
- **Triggers:** `java/net/URL.openConnection`, `.openStream`, `.getContent`; `java/net/http/HttpClient.send`, `.sendAsync`. A `URL` call isn't counted when the same method got its URL locally, from `getResource` / `getSystemResource` or `File.toURI` / `File.toURL` / `Path.toUri`: that's reading a bundled resource or a file on disk.
- **Limit:** This is decided per method, so a method that opens a local resource and a remote URL loses the remote one.

### `net.socket` · notable

**Can open raw network connections**

- **Why it matters:** A raw socket can talk any protocol to any host, or listen for incoming connections on your server.
- **Legit uses:** Vote listeners, query and RCON tools, proxies, database drivers, bundled HTTP clients.
- **Triggers:** `<init>` of `java/net/Socket`, `java/net/ServerSocket`, `java/net/DatagramSocket`, `java/net/MulticastSocket`; `java/net/Socket.connect`; `.open` on `java/nio/channels/SocketChannel`, `ServerSocketChannel`, `DatagramChannel`.

### `mc.set-op` · notable

**Can give players operator**

- **Why it matters:** An operator can run every command on the server.
- **Legit uses:** Admin and permission-management plugins, `/op` replacements, temporary-op tools.
- **Triggers:** Any method named `setOp` with descriptor `(Z)V` whose owner starts with `org/bukkit/`. Matching by name and descriptor catches calls compiled against `Player`, `OfflinePlayer`, `CommandSender` or `ServerOperator`.

### `mc.console-command` · info

**Can run server commands**

- **Why it matters:** Commands run as the console have full operator power.
- **Legit uses:** Very common. Crates, votes, rewards and shops run commands from their config.
- **Triggers:** `dispatchCommand` with descriptor `(Lorg/bukkit/command/CommandSender;Ljava/lang/String;)Z` on any `org/bukkit/` owner; `net/md_5/bungee/api/plugin/PluginManager.dispatchCommand`; `com/velocitypowered/api/command/CommandManager.executeAsync`, `.executeImmediatelyAsync`.

### `fs.write-archive` · notable

**Can write .jar or .class files**

- **Why it matters:** Writing jar or class files is how code installs or changes plugins, including other plugins.
- **Legit uses:** Self-updaters writing to `plugins/update`, plugin managers, backup plugins that create zip files, library downloaders.
- **Triggers:** `java/util/jar/JarOutputStream.<init>`; `java/util/zip/ZipOutputStream.<init>`; or, in the same method, a string literal ending in `.jar` or `.class` together with a file write: `java/io/FileOutputStream.<init>`, `java/io/RandomAccessFile.<init>`, `java/nio/file/Files.write`, `.copy`, `.move`, `.newOutputStream`, `java/nio/channels/FileChannel.open`.

### `reflect.indirect` · notable

**Reaches a powerful feature through reflection**

- **Why it matters:** Looking features up by name hides them from a quick read of the code. jarsnitch resolved the names and applied the other rules to them.
- **Legit uses:** Version-compatibility code, optional integrations, libraries that avoid hard dependencies.
- **Triggers:** String-literal arguments to `java/lang/Class.forName`, `java/lang/ClassLoader.loadClass`, `java/lang/Class.getMethod`, `.getDeclaredMethod`, `.getConstructor`, `.getDeclaredConstructor`, and `java/lang/invoke/MethodHandles$Lookup.findStatic`, `.findVirtual`, `.findConstructor`, `.findClass`. A class name and a method name in the same method resolve to that method. A class name alone counts only for classes whose use is the capability (`ProcessBuilder`, `URLClassLoader`, `ObjectInputStream`, `ScriptEngineManager`, the socket classes). Fires only when a resolved target matches another rule; ordinary reflection, such as server-version support, is not reported.
- **Limit:** Literals only. Names built at runtime are not resolved.

### `deser.java` · notable

**Reads Java serialized objects**

- **Why it matters:** If the data comes from outside (the network, a shared file, another server), crafted data can run code on your server.
- **Legit uses:** Older plugins that store data with Java serialization, caches, cross-server messaging.
- **Triggers:** `java/io/ObjectInputStream.readObject`, `.readUnshared`. Not inside a class's own `readObject(ObjectInputStream)` hook, which only reads that object's fields back.

### `script.engine` · notable

**Can run scripts**

- **Why it matters:** A script engine runs code given as text, and that text can come from a config file or the internet.
- **Legit uses:** Math in placeholders and configs, scripting add-ons.
- **Triggers:** `javax/script/ScriptEngineManager.<init>`; `javax/script/ScriptEngine.eval`.

### `proc.exit` · notable

**Can shut the server down instantly**

- **Why it matters:** It ends the Java process without a normal shutdown, so worlds and plugin data may not be saved.
- **Legit uses:** Crash handlers, license checks, launchers.
- **Triggers:** `java/lang/System.exit`; `java/lang/Runtime.exit`, `.halt`.

---

## Strings and resources

Value-carrying rules report one finding per value (one per host, IP, webhook, path), each with every location where it appears.

### `str.url` · info

**Contains web addresses**

- **Why it matters:** These are hosts the plugin may contact. Unfamiliar ones are worth a look.
- **Legit uses:** Update checks, statistics, documentation links, APIs.
- **Triggers:** Strings containing `http://`, `https://`, `ws://`, `wss://` or `ftp://`, and bare hostnames that end in a common top-level domain and don't look like a Java package or class name (`play.example.net` counts, `net.example.Foo` doesn't). The value is the host. A text file with more than 100 bare hostnames is taken for a domain list, such as the public suffix list bundled with Apache HttpClient: hostnames in it aren't reported, and a `text.host-list` warning names the file.

### `str.ip` · notable

**Contains a hardcoded IP address**

- **Why it matters:** A fixed address skips DNS, which makes it harder to tell who it belongs to.
- **Legit uses:** Rare. Some network tools, server lists, DNS fallbacks.
- **Triggers:** Standalone IPv4 literals, optionally with a port, and bracketed IPv6 in URLs. Not counted: parts of longer dotted runs (version numbers), values after `rv:`, and private, loopback, link-local, carrier-grade NAT, multicast, reserved and unspecified ranges. Documentation ranges (RFC 5737) are reported like public addresses so test fixtures can use them.
- **Not addresses:** a bare IPv4 without a port is also skipped when it's more likely something else: a Maven version after `:` (`sqlite-jdbc:3.42.0.1`), an address ending in `.0`, an ASN.1 object id (first part 1 or 2 and second below 40, like `2.5.29.14`), or a number whose four parts are all below 16 (`4.2.1.10`, section and version numbers). These still count inside a URL or with a port. Well-known resolvers like `1.1.1.1` and `8.8.8.8` fall under these rules and only show up with a port or in a URL.

### `str.discord-webhook` · notable

**Can send data to a Discord webhook**

- **Why it matters:** A webhook built into the code posts to a Discord channel the author controls, not you.
- **Legit uses:** Author error reporting. A plugin that lets *you* set a webhook in its config usually contains only the URL prefix, which is reported under `str.url` instead.
- **Triggers:** `discord.com`, `discordapp.com`, `ptb.discord.com` or `canary.discord.com` followed by `/api/webhooks/<id>/<token>` (optionally `/api/v<N>/`), with both id and token present. The value is the webhook id. The token is redacted to its first 4 characters so reports can be shared; the id is kept so the webhook can be reported to Discord.

### `str.telegram-bot` · notable

**Can send data to a Telegram bot**

- **Why it matters:** A bot token built into the code lets the plugin message a chat the author controls.
- **Legit uses:** Author notifications. Rare in public plugins.
- **Triggers:** `api.telegram.org/bot<id>:<token>`. The value is the bot id; the token is redacted like webhook tokens.

### `str.hidden-text` · notable

**Contains text hidden from simple searches**

- **Why it matters:** The code writes this text as numbers or an encoded string, so it doesn't appear in the jar as plain text. jarsnitch decoded it and checked it like any other text.
- **Legit uses:** Obfuscators used to protect paid plugins. A few libraries store small constants this way.
- **Triggers:** `new String(...)`, `String.valueOf` or `String.copyValueOf` on a constant byte or char array of 3 or more readable characters, as [documented for real samples](https://github.com/trigram-mrp/fractureiser/blob/main/docs/tech.md). `new String(Base64.getDecoder().decode("..."))` on a literal. Any base64 or hex string of 16 to 4,096 characters that decodes to text with a URL or IP in it. Every other rule sees the decoded text: an address counts as `str.url` or `str.ip`, and a class name counts for reflection and correlations.
- **Not counted:** lowercase names like `org.bstats`, which libraries write as bytes so that shading doesn't rewrite them (reflection still sees these), and skin textures, which are base64 JSON with a `textures.minecraft.net` URL.
- **Limit:** Only constant values are read. Text that's XORed, encrypted, split across methods or computed at runtime isn't decoded.

### `str.login-file` · notable

**Contains the path of a password or login file**

- **Why it matters:** These files hold logins for browsers, game launchers or SSH. A Minecraft plugin rarely has a reason to name them.
- **Legit uses:** Rare. SSH and SFTP libraries look for key files; backup tools may list paths to skip.
- **Triggers:** Text containing, in any case and with either slash: `Local Storage/leveldb`, `/Login Data`, `/Network/Cookies`, `cookies.sqlite`, `key4.db`, `launcher_accounts.json`, `launcher_msa_credentials`, `microsoft_accounts.json`, `wallet.dat`, `.ssh/id_`, `.aws/credentials`, `.git-credentials` or `/etc/shadow`. A string that is exactly `id_rsa` or `id_ed25519` also counts. The value is the matched text.

### `blob.embedded-code` · notable

**Contains hidden embedded code**

- **Why it matters:** Code stored as encoded text doesn't show up as a normal class file. jarsnitch decoded it and scanned it; its findings carry the location `<class>!embedded-<n>`.
- **Legit uses:** Rare. Some license systems and obfuscators.
- **Triggers:** Base64 (standard or URL-safe) or hex strings of 64 or more characters that decode to a class file (`CAFEBABE`) or a zip archive. Decoded archives are scanned like nested jars. The value is the SHA-256 of the decoded bytes.
- **Limit:** A payload split across several strings and joined at runtime is not reassembled.

### `res.type-mismatch` · notable

**A file's contents don't match its name**

- **Why it matters:** A file named like an image or a config that actually holds code or a program isn't there to be an image or a config.
- **Legit uses:** Rare. Some build tools store data under odd extensions.
- **Triggers:** An entry whose bytes are a class file, zip, ELF, PE (`MZ`) or Mach-O binary while its extension says otherwise (`.png`, `.yml`, `.dat`, no extension, ...), and a `.class` entry that isn't a class file. Archives named `.jar`, `.zip`, `.war`, `.jarinjar` (LuckPerms-style loaders) or `.mcpack` (Bedrock resource packs) and native libraries named `.so`, `.dll`, `.dylib`, `.jnilib`, `.exe`, `.bin` or `.node` are fine. Disguised classes and archives are scanned as well. The value is the entry path.

### `jar.nested` · info

**Bundles other archives**

- **Why it matters:** Nested archives are scanned too, up to 3 levels deep, and their findings are listed with the jar path.
- **Legit uses:** Very common: bundled libraries, jar-in-jar loaders, multi-version modules, bundled resource packs.
- **Triggers:** Entries ending in `.jar` or `.zip`, or whose bytes are a zip. The value is the entry path.

### `jar.unsafe-path` · notable

**Archive has file paths that point outside it**

- **Why it matters:** A path like `../../start.sh` can overwrite files on your server if a tool extracts the jar. Java never needs one.
- **Legit uses:** None known. Usually a broken zip tool.
- **Triggers:** Entry names with `..` segments, a leading `/`, a drive letter or backslashes. jarsnitch never extracts files; these entries are still scanned under a cleaned name. The value is the original name.

---

## Plugin metadata

Descriptors read into the report's `plugins` section: `plugin.yml`, `paper-plugin.yml`, `bungee.yml`, `velocity-plugin.json`. Fields: name, version, main (and Paper's `bootstrapper` and `loader`), commands, permissions, `folia-supported`, and the Maven libraries the server downloads for the plugin (`libraries:`). A multi-platform jar can contain several descriptors; all are listed.

Mods get name and version only, from `fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml` or `META-INF/neoforge.mods.toml`.

Bundled libraries come from `META-INF/maven/<group>/<artifact>/pom.properties`, in the jar and in nested jars.

### `meta.missing-main` · notable

**The declared main class is missing**

- **Why it matters:** The server loads this class first. If it isn't in the jar, the plugin either fails to load or gets its code from somewhere jarsnitch can't see.
- **Legit uses:** Broken builds, a jar meant for another platform.
- **Triggers:** `main` (or Paper `bootstrapper` / `loader`) in a Bukkit, Paper, BungeeCord or Velocity descriptor naming a class that isn't in the jar. The value is the class name.

### `meta.no-descriptor` · info

**Not a recognized plugin**

- **Why it matters:** No plugin.yml or similar file was found. This may be a library, an API jar or not a Minecraft jar at all.
- **Legit uses:** Libraries and API jars.
- **Triggers:** None of the plugin or mod descriptors above is present.

### `lib.log4j-vulnerable` · high

**Bundles a log4j version with known security bugs**

- **Why it matters:** log4j-core before 2.17.1 has known bugs, including Log4Shell (CVE-2021-44228), that can let other people run code on your server.
- **Legit uses:** None. It's an old build; ask the author for an update.
- **Triggers:** `org.apache.logging.log4j:log4j-core` below 2.17.1, except the patched backports 2.12.4+ (Java 7) and 2.3.2+ (Java 6). Source: <https://logging.apache.org/security.html>
- **Limit:** Found only when the bundled copy keeps its `pom.properties`. Relocated copies without it are missed.

---

## Obfuscation

### `obf.score` · info

**Code is obfuscated (score N/100)**

- **Why it matters:** Obfuscation makes code hard to read, for people and for jarsnitch. Findings may be incomplete.
- **Legit uses:** Common in paid plugins to protect the author's work. Not a problem by itself.
- **Triggers:** The score is always in the report; the finding appears at 25 or more.

```text
score = round(100 × min(1, 0.5·S + 0.2·P + 0.3·E))
```

- **S:** share of class, method and field names that are 1–2 characters long, contain characters outside `A–Z a–z 0–9 _ $`, or are Java keywords. Ignores `<init>`, `<clinit>`, `lambda$…`, `access$…`, `values`, `valueOf`.
- **P:** share of classes in the default package.
- **E:** share of string literals of 16+ characters that look encoded: hex of 32+ characters; base64 of 20+ characters mixing upper case, lower case and digits, with no lower-case run of 7 or more, whose decoded start isn't printable text; or 32+ characters with entropy above 4.8 bits per character.

Small jars are damped so that a handful of names can't produce a high score: in the formula, S divides by at least 20 names, P by at least 5 classes and E by at least 20 long strings. The report's `obfuscation` section shows the undamped shares.

Computed over the top-level jar's own classes, not nested jars. The weights and threshold are starting values, tuned with `scripts/fp-bench`.

---

## Correlations

Each raises a High finding when all its parts are present. Scope **jar** means the whole scanned file, including nested jars and decoded code. Scope **class** means one class. Scope **class or call** adds one method together with the methods it calls directly, so a method that downloads through one class and loads code through another counts; the evidence is placed at that method and names where each part is. Evidence lists the parts that matched.

The network correlations use class scope because large plugins bundle libraries (database drivers, dependency loaders) whose network and process code sits in unrelated classes; on the whole jar they fired on most popular plugins. A host or IP string counts as the network part so that `new URLClassLoader(new URL[] { new URL("http://...") })` is caught, including when the string is [hidden](#strhidden-text--notable). **Limit:** parts joined only through longer call chains, fields or callbacks don't correlate; their individual findings still appear.

### `corr.download-and-run` · high

**Can download and run code from the internet**

- **Parts:** `net.http`, `net.socket`, a `str.url` host or a `str.ip` address, plus `code.load`. Scope: class or call.
- **Why it matters:** The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.
- **Legit uses:** Dependency downloaders that fetch libraries from Maven repositories at startup. Check the hosts under `str.url`.

### `corr.hidden-command` · high

**Possible hidden command**

- **Parts, all in one class:**
  1. an event handler (a method whose only parameter is the event) for chat or command input: `org/bukkit/event/player/AsyncPlayerChatEvent`, `PlayerChatEvent`, `PlayerCommandPreprocessEvent`, `io/papermc/paper/event/player/AsyncChatEvent`, `net/md_5/bungee/api/event/ChatEvent`, `com/velocitypowered/api/event/player/PlayerChatEvent`, `com/velocitypowered/api/event/command/CommandExecuteEvent`;
  2. a comparison against a string literal: `String.equals`, `.equalsIgnoreCase`, `.startsWith`, `.endsWith`, `.contains`, or a `switch` on a string;
  3. `mc.set-op` or `mc.console-command`.
- **Why it matters:** Typing a specific word in chat could give someone operator or run console commands, without that command appearing in the plugin's command list.
- **Legit uses:** Chat shortcuts and command aliases. Real features usually read their trigger words from the config instead of hardcoding them.

### `corr.modify-plugins` · high

**Can modify other plugins**

- **Parts, in one class:**
  1. `fs.write-archive` evidence from `JarOutputStream` or a `.jar` / `.class` file write. A plain `ZipOutputStream` doesn't count, so backup plugins don't trigger this.
  2. A reference to the plugins folder: a string literal `plugins` or one starting with `plugins/` or `plugins\`; `getPluginsFolder`, `getUpdateFolder` or `getUpdateFolderFile` on an `org/bukkit/` owner; or `getDataFolder` followed by `java/io/File.getParentFile` in the same method.
- **Why it matters:** Code that writes jar files into the plugins folder can replace or alter other plugins. It's a known way for malicious code to spread between plugins.
- **Legit uses:** Self-updaters, plugin managers.

### `corr.command-and-network` · high

**Can run operating-system commands and connect to the internet**

- **Parts:** `exec.process`, plus `net.http`, `net.socket`, a `str.url` host or a `str.ip` address. Scope: class or call.
- **Why it matters:** Together these allow remote control of the machine: commands could arrive over the network and results go back out.
- **Legit uses:** Hosting-panel integrations, plugins that download and run helper tools.

### `corr.login-files-and-network` · high

**Names password or login files and can connect to the internet**

- **Parts:** `str.login-file`, plus `net.http`, `net.socket`, a `str.url` host or a `str.ip` address. Scope: class or call.
- **Why it matters:** One class has both the path of a login file and network access, which is what sending that file somewhere would take.
- **Legit uses:** Rare. SSH and SFTP libraries that load a key file and open a connection.

### `corr.hidden-loader` · high

**Contains hidden code and can load it**

- **Parts:** `blob.embedded-code` plus `code.load`. Scope: jar.
- **Why it matters:** Encoded code plus the ability to load it is a common way to keep code out of sight.
- **Legit uses:** Some license systems.

---

## Known-bad indicators

v1 doesn't compare jars against a list of known file hashes or hosts. If one is added, every entry needs a public source URL next to it. jarsnitch never invents signatures. See [CONTRIBUTING.md](../CONTRIBUTING.md#known-bad-indicators).

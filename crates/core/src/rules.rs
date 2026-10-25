//! Rule table: ids, severities and copy, plus the call signatures that trigger them.
//! docs/rules.md must match this file.

use crate::Severity::{self, High, Info, Notable};

#[derive(Debug)]
pub struct Rule {
    pub id: &'static str,
    pub severity: Severity,
    pub title: &'static str,
    pub why: &'static str,
    pub legit_uses: &'static str,
}

const fn r(id: &'static str, severity: Severity, title: &'static str, why: &'static str, legit_uses: &'static str) -> Rule {
    Rule { id, severity, title, why, legit_uses }
}

pub const RULES: &[Rule] = &[
    r("exec.process", Notable, "Can run commands on your server's operating system",
      "The code can start any program the server's OS account can run. That is access to the whole machine, not just Minecraft.",
      "Running bundled tools (ffmpeg, git), backup plugins calling tar or zip, hosting-panel helpers."),
    r("code.load", Notable, "Can load new code while running",
      "Code that isn't in this jar can be added later, and jarsnitch can only judge what's in the file.",
      "Dependency loaders that fetch libraries at startup, addon or module systems, scripting add-ons."),
    r("code.instrument", Notable, "Can change other code as it loads",
      "A Java agent can rewrite the server's classes and other plugins' classes.",
      "Profilers, hot-reload tools, some compatibility patchers."),
    r("code.native", Notable, "Can load native (non-Java) code",
      "Native code runs outside Java's checks, and jarsnitch can't inspect it.",
      "Profilers, compression libraries, database drivers such as SQLite."),
    r("net.http", Info, "Can connect to websites",
      "The plugin can send and receive data over the internet. The hosts found in the jar are listed under str.url.",
      "Update checks, license checks, usage statistics (bStats), web APIs, Discord integrations."),
    r("net.socket", Notable, "Can open raw network connections",
      "A raw socket can talk any protocol to any host, or listen for incoming connections on your server.",
      "Vote listeners, query and RCON tools, proxies, database drivers, bundled HTTP clients."),
    r("mc.set-op", Notable, "Can give players operator",
      "An operator can run every command on the server.",
      "Admin and permission-management plugins, /op replacements, temporary-op tools."),
    r("mc.console-command", Info, "Can run server commands",
      "Commands run as the console have full operator power.",
      "Very common. Crates, votes, rewards and shops run commands from their config."),
    r("fs.write-archive", Notable, "Can write .jar or .class files",
      "Writing jar or class files is how code installs or changes plugins, including other plugins.",
      "Self-updaters writing to plugins/update, plugin managers, backup plugins that create zip files, library downloaders."),
    r("reflect.indirect", Notable, "Reaches a powerful feature through reflection",
      "Looking features up by name hides them from a quick read of the code. jarsnitch resolved the names and applied the other rules to them.",
      "Version-compatibility code, optional integrations, libraries that avoid hard dependencies."),
    r("deser.java", Notable, "Reads Java serialized objects",
      "If the data comes from outside (the network, a shared file, another server), crafted data can run code on your server.",
      "Older plugins that store data with Java serialization, caches, cross-server messaging."),
    r("script.engine", Notable, "Can run scripts",
      "A script engine runs code given as text, and that text can come from a config file or the internet.",
      "Math in placeholders and configs, scripting add-ons."),
    r("proc.exit", Notable, "Can shut the server down instantly",
      "It ends the Java process without a normal shutdown, so worlds and plugin data may not be saved.",
      "Crash handlers, license checks, launchers."),
    r("str.url", Info, "Contains web addresses",
      "These are hosts the plugin may contact. Unfamiliar ones are worth a look.",
      "Update checks, statistics, documentation links, APIs."),
    r("str.ip", Notable, "Contains a hardcoded IP address",
      "A fixed address skips DNS, which makes it harder to tell who it belongs to.",
      "Rare. Some network tools, server lists, DNS fallbacks."),
    r("str.discord-webhook", Notable, "Can send data to a Discord webhook",
      "A webhook built into the code posts to a Discord channel the author controls, not you.",
      "Author error reporting. A plugin that lets you set a webhook in its config usually contains only the URL prefix, which is reported under str.url instead."),
    r("str.telegram-bot", Notable, "Can send data to a Telegram bot",
      "A bot token built into the code lets the plugin message a chat the author controls.",
      "Author notifications. Rare in public plugins."),
    r("blob.embedded-code", Notable, "Contains hidden embedded code",
      "Code stored as encoded text doesn't show up as a normal class file. jarsnitch decoded it and scanned it; its findings carry the location <class>!embedded-<n>.",
      "Rare. Some license systems and obfuscators."),
    r("res.type-mismatch", Notable, "A file's contents don't match its name",
      "A file named like an image or a config that actually holds code or a program isn't there to be an image or a config.",
      "Rare. Some build tools store data under odd extensions."),
    r("jar.nested", Info, "Bundles other archives",
      "Nested archives are scanned too, up to 3 levels deep, and their findings are listed with the jar path.",
      "Very common: bundled libraries, jar-in-jar loaders, multi-version modules, bundled resource packs."),
    r("jar.unsafe-path", Notable, "Archive has file paths that point outside it",
      "A path like ../../start.sh can overwrite files on your server if a tool extracts the jar. Java never needs one.",
      "None known. Usually a broken zip tool."),
    r("meta.missing-main", Notable, "The declared main class is missing",
      "The server loads this class first. If it isn't in the jar, the plugin either fails to load or gets its code from somewhere jarsnitch can't see.",
      "Broken builds, a jar meant for another platform."),
    r("meta.no-descriptor", Info, "Not a recognized plugin",
      "No plugin.yml or similar file was found. This may be a library, an API jar or not a Minecraft jar at all.",
      "Libraries and API jars."),
    r("lib.log4j-vulnerable", High, "Bundles a log4j version with known security bugs",
      "log4j-core before 2.17.1 has known bugs, including Log4Shell (CVE-2021-44228), that can let other people run code on your server.",
      "None. It's an old build; ask the author for an update."),
    r("obf.score", Info, "Code is obfuscated",
      "Obfuscation makes code hard to read, for people and for jarsnitch. Findings may be incomplete.",
      "Common in paid plugins to protect the author's work. Not a problem by itself."),
    r("corr.download-and-run", High, "Can download and run code from the internet",
      "The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.",
      "Dependency downloaders that fetch libraries from Maven repositories at startup. Check the hosts under str.url."),
    r("corr.hidden-command", High, "Possible hidden command",
      "Typing a specific word in chat could give someone operator or run console commands, without that command appearing in the plugin's command list.",
      "Chat shortcuts and command aliases. Real features usually read their trigger words from the config instead of hardcoding them."),
    r("corr.modify-plugins", High, "Can modify other plugins",
      "Code that writes jar files into the plugins folder can replace or alter other plugins. It's a known way for malicious code to spread between plugins.",
      "Self-updaters, plugin managers."),
    r("corr.command-and-network", High, "Can run operating-system commands and connect to the internet",
      "Together these allow remote control of the machine: commands could arrive over the network and results go back out.",
      "Hosting-panel integrations, plugins that download and run helper tools."),
    r("corr.hidden-loader", High, "Contains hidden code and can load it",
      "Encoded code plus the ability to load it is a common way to keep code out of sight.",
      "Some license systems."),
];

/// Looks up a rule by id. Panics only on an id missing from the table, which is a bug in this crate.
pub fn rule(id: &str) -> &'static Rule {
    RULES.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("unknown rule id {id}"))
}

pub enum Owner {
    Exact(&'static str),
    Prefix(&'static str),
}

pub struct Call {
    pub rule: &'static str,
    pub owner: Owner,
    /// `*` matches any method name.
    pub name: &'static str,
    pub desc: Option<&'static str>,
}

const fn c(rule: &'static str, owner: &'static str, name: &'static str) -> Call {
    Call { rule, owner: Owner::Exact(owner), name, desc: None }
}

const CONSOLE_DESC: &str = "(Lorg/bukkit/command/CommandSender;Ljava/lang/String;)Z";

pub const CALLS: &[Call] = &[
    c("exec.process", "java/lang/Runtime", "exec"),
    c("exec.process", "java/lang/ProcessBuilder", "<init>"),
    c("exec.process", "java/lang/ProcessBuilder", "start"),
    c("exec.process", "java/lang/ProcessBuilder", "startPipeline"),
    c("code.load", "java/net/URLClassLoader", "<init>"),
    c("code.load", "java/net/URLClassLoader", "newInstance"),
    c("code.load", "java/net/URLClassLoader", "defineClass"),
    c("code.load", "java/lang/ClassLoader", "defineClass"),
    c("code.load", "java/security/SecureClassLoader", "defineClass"),
    c("code.load", "java/lang/invoke/MethodHandles$Lookup", "defineClass"),
    c("code.load", "java/lang/invoke/MethodHandles$Lookup", "defineHiddenClass"),
    c("code.load", "sun/misc/Unsafe", "defineClass"),
    c("code.load", "sun/misc/Unsafe", "defineAnonymousClass"),
    c("code.load", "jdk/internal/misc/Unsafe", "defineClass"),
    c("code.instrument", "java/lang/instrument/Instrumentation", "*"),
    c("code.instrument", "com/sun/tools/attach/VirtualMachine", "attach"),
    c("code.instrument", "com/sun/tools/attach/VirtualMachine", "loadAgent"),
    c("code.native", "java/lang/System", "load"),
    c("code.native", "java/lang/System", "loadLibrary"),
    c("code.native", "java/lang/Runtime", "load"),
    c("code.native", "java/lang/Runtime", "loadLibrary"),
    c("code.native", "java/lang/foreign/SymbolLookup", "libraryLookup"),
    c("net.http", "java/net/URL", "openConnection"),
    c("net.http", "java/net/URL", "openStream"),
    c("net.http", "java/net/URL", "getContent"),
    c("net.http", "java/net/http/HttpClient", "send"),
    c("net.http", "java/net/http/HttpClient", "sendAsync"),
    c("net.socket", "java/net/Socket", "<init>"),
    c("net.socket", "java/net/Socket", "connect"),
    c("net.socket", "java/net/ServerSocket", "<init>"),
    c("net.socket", "java/net/DatagramSocket", "<init>"),
    c("net.socket", "java/net/MulticastSocket", "<init>"),
    c("net.socket", "java/nio/channels/SocketChannel", "open"),
    c("net.socket", "java/nio/channels/ServerSocketChannel", "open"),
    c("net.socket", "java/nio/channels/DatagramChannel", "open"),
    Call { rule: "mc.set-op", owner: Owner::Prefix("org/bukkit/"), name: "setOp", desc: Some("(Z)V") },
    Call { rule: "mc.console-command", owner: Owner::Prefix("org/bukkit/"), name: "dispatchCommand", desc: Some(CONSOLE_DESC) },
    c("mc.console-command", "net/md_5/bungee/api/plugin/PluginManager", "dispatchCommand"),
    c("mc.console-command", "com/velocitypowered/api/command/CommandManager", "executeAsync"),
    c("mc.console-command", "com/velocitypowered/api/command/CommandManager", "executeImmediatelyAsync"),
    c("fs.write-archive", "java/util/jar/JarOutputStream", "<init>"),
    c("fs.write-archive", "java/util/zip/ZipOutputStream", "<init>"),
    c("deser.java", "java/io/ObjectInputStream", "readObject"),
    c("deser.java", "java/io/ObjectInputStream", "readUnshared"),
    c("script.engine", "javax/script/ScriptEngineManager", "<init>"),
    c("script.engine", "javax/script/ScriptEngine", "eval"),
    c("proc.exit", "java/lang/System", "exit"),
    c("proc.exit", "java/lang/Runtime", "exit"),
    c("proc.exit", "java/lang/Runtime", "halt"),
];

/// Rule ids triggered by a call. `desc: None` ignores descriptors (used for reflection,
/// where only owner and name are known).
pub fn match_call<'a>(owner: &'a str, name: &'a str, desc: Option<&'a str>) -> impl Iterator<Item = &'static str> + 'a {
    CALLS.iter().filter_map(move |c| {
        let name_ok = c.name == "*" || c.name == name;
        let owner_ok = match c.owner {
            Owner::Exact(o) => o == owner,
            Owner::Prefix(p) => owner.starts_with(p),
        };
        let desc_ok = match (c.desc, desc) {
            (Some(want), Some(got)) => want == got,
            _ => true,
        };
        (name_ok && owner_ok && desc_ok).then_some(c.rule)
    })
}

/// True if some rule (other than constructors) matches this method name, so a call on an
/// in-jar subclass is worth resolving against the superclass chain later.
pub fn is_inheritable_name(name: &str) -> bool {
    name != "<init>" && CALLS.iter().any(|c| c.name == name)
}

/// Classes whose mere use through `Class.forName` is the capability.
pub const REFLECT_CLASS_ONLY: &[(&str, &str)] = &[
    ("java/lang/ProcessBuilder", "exec.process"),
    ("java/net/URLClassLoader", "code.load"),
    ("java/io/ObjectInputStream", "deser.java"),
    ("javax/script/ScriptEngineManager", "script.engine"),
    ("java/net/Socket", "net.socket"),
    ("java/net/ServerSocket", "net.socket"),
    ("java/net/DatagramSocket", "net.socket"),
    ("java/net/MulticastSocket", "net.socket"),
];

/// Calls that write a file, for the `.jar` / `.class` literal check.
pub fn is_file_write(owner: &str, name: &str) -> bool {
    matches!(
        (owner, name),
        ("java/io/FileOutputStream", "<init>")
            | ("java/io/RandomAccessFile", "<init>")
            | ("java/nio/file/Files", "write" | "copy" | "move" | "newOutputStream")
            | ("java/nio/channels/FileChannel", "open")
    )
}

/// Single-parameter event handlers that see chat or command text.
pub const CHAT_HANDLER_DESCS: &[&str] = &[
    "(Lorg/bukkit/event/player/AsyncPlayerChatEvent;)V",
    "(Lorg/bukkit/event/player/PlayerChatEvent;)V",
    "(Lorg/bukkit/event/player/PlayerCommandPreprocessEvent;)V",
    "(Lio/papermc/paper/event/player/AsyncChatEvent;)V",
    "(Lnet/md_5/bungee/api/event/ChatEvent;)V",
    "(Lcom/velocitypowered/api/event/player/PlayerChatEvent;)V",
    "(Lcom/velocitypowered/api/event/command/CommandExecuteEvent;)V",
];

pub const STRING_COMPARES: &[&str] = &["equals", "equalsIgnoreCase", "startsWith", "endsWith", "contains"];

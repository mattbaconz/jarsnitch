# Read a report

A report lists what a plugin's code is able to do, how much that matters and where in the code it sits. It doesn't tell you whether to install the plugin; that judgement stays with you.

## Three rules first

- **"Can" means the code is in the jar, not that it runs.** A finding says the instructions are there. Whether and when they run depends on the plugin and its config.
- **Severity comes from the rule.** Info is common in ordinary plugins, notable is a powerful capability worth checking, high is a combination or a vulnerable library that gives the jar a lot of control. See [severities](../rules.md#severities).
- **"Not called" never lowers a severity.** Findings in code the plugin's entry points don't call are marked and listed last, with the same severity. See [The call graph](call-graph.md).

## Anatomy of a finding

<!-- diagram: finding -->

1. **Severity:** info, notable or high.
2. **Title** in plain English. Rules that carry a value add it after a colon, such as `Contains web addresses: api.example.invalid`.
3. **Rule id.** It never changes, so you can filter on it in `--json` output. On the website it links to the rule.
4. **Why it matters,** in one line.
5. **Trail:** on the website, how the plugin gets to the code, from an entry point (or from the jar) down to the call or text that matched. See [The trail](#the-trail). The command line prints the first 3 places as `at` lines, then "+N more".
6. **Not called:** nothing the plugin starts from reaches this code. On the website the trail shows a dashed `no caller` break instead of a call path; the command line adds "not called from the plugin's entry points".
7. **Common legitimate uses,** because most capabilities have them. The website and the JSON carry this line; the command line leaves it out to stay short.

## The trail

Each card for a single finding on the website has a trail: a short list of rows down to the place the rule matched. A row starts with a label. For the steps of a call path (`entry`, `calls`, `callback`, `static init`, `reflection`) and for `embedded`, point at the label for what it means, or, with a screen reader, hear it read with the label. The table below explains every label.

Each row has a node on the trail's line, drawn like the marks in [Where in the jar](#where-in-the-jar): a solid square for code the plugin's entry points reach, a hollow square for code nothing calls, and a circle for the jar, a nested jar, a file or decoded code, which the call graph doesn't cover. The `match` node takes the severity's colour.

| Row | Meaning |
|---|---|
| `jar` | The file you scanned. A trail without a call path starts here. |
| `entry` | Where the server starts the plugin: a method of its main class, or of a class it declares, such as `onEnable`. |
| `calls` | The row above calls this method, directly or through an interface or superclass. |
| `callback` | The server, a library or the JVM calls this method on an object the row above creates: an event handler, a command, `run()`, `toString()`. |
| `static init` | The row above uses the class, which runs its static initializer, shown as `.<clinit>`. |
| `reflection` | The row above names the class for reflection, or lists its methods. |
| `no caller` | A dashed break: nothing the entry points run calls this code. |
| `nested jar` | A jar inside the jar. The rows below it are inside that jar. |
| `class` | A class, with the method when the place is inside one. |
| `embedded` | Code stored as encoded text in the class above, decoded and scanned. |
| `file` | A file in the jar, such as `plugin.yml`. |
| `match` | The call or text that matched, up to 4 from the same place. In a combination each part starts with its rule id, such as `code.load`. |

A method is shown as package, class and method with short parameter types, like `example/sampleshop/ChatListener.onChat(AsyncPlayerChatEvent)`. A constructor is shown as `new ClassName(…)`.

Which rows a trail has depends on the finding:

- **Reached:** the call path the engine found, from an `entry` row to the method the finding is in, then the matches there. A path of more than 7 steps shows the first 2 and the last 4, with a button for the steps between. How the path is chosen: [Call paths](call-graph.md#call-paths).
- **Not called:** `jar`, the dashed `no caller` break, then the class and method, then the matches.
- **Not code, or no call graph:** `jar`, then where the place is: each `nested jar` it sits in, then the `class` (with an `embedded` row for decoded code) or the `file`, then the matches. A finding about the jar as a whole, like the obfuscation score, goes straight from `jar` to the match.

Two trails from the SampleShop 1.4.1 sample, written out as text:

```text
entry      example/sampleshop/SampleShop.onEnable()
callback   example/sampleshop/ChatListener.onChat(AsyncPlayerChatEvent)
match      org/bukkit/entity/Player.setOp
```

```text
jar        SampleShop-1.4.1.jar
no caller  Nothing the entry points run calls this.
class      example/sampleshop/libs/net/TcpClient.connect(String, int)
match      java/net/Socket.<init>
```

› `mc.set-op` and `net.socket` in the sample, as the website draws them

When a finding was found in more places than its trail shows, **All N places it was found** under the card opens a list of them: up to 50, each with its class and method, the matched call or text, and "not called" when it applies, then "N more not listed" for the rest.

Findings that share a rule and differ only in value share one card that lists the values instead of a trail; see [Findings](#findings) below.

## The website's report, top to bottom

### Summary

On the left, or at the top on a phone:

- **Title:** the plugin's name and version, or the file name when the jar has no descriptor.
- **Meta line:** file name, platform (`bukkit plugin`), size, the first 12 characters of the SHA-256, class count and `obfuscation N/100`.
- **Counts** of High, Notable and Info findings.
- **Download JSON**, the full report as a file.
- **In this report:** links to each section.
- **Reading this report:** a short legend for the three severities.

### Banners

A banner at the top when it applies: the sample note, "This file isn't a readable jar or zip, so nothing in it was checked.", or "Partial report: a scan limit was hit, so part of the jar wasn't checked."

### Where in the jar

A map of where the findings are, above the findings. It's there when at least one finding is at a place inside the jar.

- **Call graph meter:** "Call graph: R of M methods reached from N entry points", with a bar filled to that share. Only for a jar with a call graph.
- **Jar map:** a tree with a row for each package with findings, like `example/sampleshop/`, and under it a row for each class or file with findings. Files at the top level of the jar, like `plugin.yml`, sit under the jar's file name.
- **Marks:** one for each rule found at that place, highest severity first, with the severity's letter (H, N or I) in its colour. A square is code the call graph covers: solid when the entry points reach it, hollow when nothing they run calls it. A circle is a place it doesn't cover: a file, a class in a nested jar, or decoded code. The rule ids follow the marks, except on a phone. Each mark's severity, rule and reach are also given as a tooltip and as text for screen readers, and a key under the map explains the marks it uses.
- **Links:** a class or file name links to the card of its highest finding, and opens the Info fold first when that card is inside it.
- **Up to 16 places.** A jar with more shows the 16 with the highest severity, then the most rules, and counts the rest: "and N more places with fewer or lower findings".

In the SampleShop 1.4.1 sample the map has 6 places: `plugin.yml`, with a circle for `str.url`; `AddonLoader` with 4 solid squares (`corr.download-and-run`, `code.load`, `fs.write-archive`, `str.url`), `ChatListener` with 2, `ShopCommand` with 1 and `UpdateChecker` with 2, all in `example/sampleshop/`; and `TcpClient` in `example/sampleshop/libs/net/`, with one hollow square for `net.socket`.

### Findings

- High and Notable findings as cards, highest first, with the ones in code nothing calls at the end of the list. A card for one finding has a [trail](#the-trail).
- When some findings aren't called *and* the plugin loads code at runtime, a line above the cards says that code may still run.
- **Info findings (n)** are folded; open the fold to see them.
- Findings that share a rule and differ only in value, like several hosts, share one card titled with the count, such as "Contains web addresses (3)". Each value shows where it was found, as "in X and N more places"; past 8 values a **Show N more** button lists the rest. When those findings aren't called, the card ends with "Not called from the plugin's entry points."
- An empty section says "No high or notable findings." or, when nothing at all matched, "jarsnitch found nothing from its rule list in this jar."

### About this jar

- **File, Size, SHA-256, Classes, Entries.**
- **Obfuscation:** the score, plus the share of short or garbled names, of classes in the default package and of encoded strings. How the score is computed: [`obf.score`](../rules.md#obfscore--info).
- **Call graph:** "R of M methods reached from N entry points", and the first 3 entry points.
- **One block per descriptor,** such as `plugin.yml (bukkit)`: name, version, authors, website, main class, Paper's bootstrapper and loader, Folia support, commands, permissions, dependencies and optional dependencies.
- **Libraries the server downloads at startup** (the descriptor's `libraries:`), **bundled libraries** (from `pom.properties`) and **nested jars**.

### Warnings

**Warnings (n)** lists problems reading the jar, each with its code, where it happened and a message. The codes are listed [below](#warning-codes).

### Several jars

Scanning several jars gives one row per jar, most High findings first, then most Notable. Each row opens to the full report described above.

### A comparison

A comparison has its own layout: what the update adds, removes and changes, then what's unchanged. The findings it adds get the same cards, trails included, drawn from the new version. See [Compare two versions](compare.md#reading-a-comparison).

## The command line report, line by line

```out $ jarsnitch scan SampleShop-1.4.1.jar
== SampleShop-1.4.1.jar ==
SampleShop 1.4.1 · bukkit (plugin.yml)
sha256 699600d2ac6a · 6 classes · obfuscation 0/100
call graph from example/sampleshop/SampleShop: 10 of 12 methods reached
  the plugin also loads code at runtime, so code it doesn't reach may still run

HIGH     Can download and run code from the internet   [corr.download-and-run]
         The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.
         at example/sampleshop/AddonLoader.load

HIGH     Possible hidden command   [corr.hidden-command]
         Typing a specific word in chat could give someone operator or run console commands, without that command appearing in the plugin's command list.
         at example/sampleshop/ChatListener

NOTABLE  Can load new code while running   [code.load]
         Code that isn't in this jar can be added later, and jarsnitch can only judge what's in the file.
         at example/sampleshop/AddonLoader.load

NOTABLE  Can write .jar or .class files   [fs.write-archive]
         Writing jar or class files is how code installs or changes plugins, including other plugins.
         at example/sampleshop/AddonLoader.load

NOTABLE  Can give players operator   [mc.set-op]
         An operator can run every command on the server.
         at example/sampleshop/ChatListener.onChat

INFO     Can run server commands   [mc.console-command]
         Commands run as the console have full operator power.
         at example/sampleshop/ShopCommand.onCommand

INFO     Can connect to websites   [net.http]
         The plugin can send and receive data over the internet. The hosts found in the jar are listed under str.url.
         at example/sampleshop/UpdateChecker.check

INFO     Contains web addresses: api.example.invalid   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at example/sampleshop/UpdateChecker.check

INFO     Contains web addresses: cdn.example.invalid   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at example/sampleshop/AddonLoader.load

INFO     Contains web addresses: shop-plugins.example.net   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at plugin.yml

NOTABLE  Can open raw network connections   [net.socket]
         A raw socket can talk any protocol to any host, or listen for incoming connections on your server.
         at example/sampleshop/libs/net/TcpClient.connect
         not called from the plugin's entry points

2 high · 4 notable · 5 info · 1 not called from entry points

This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.
```

› real output: jarsnitch 0.2.1 on the site's sample jar · exit code 2

| Line | Meaning |
|---|---|
| `== SampleShop-1.4.1.jar ==` | The file name, without its folder. |
| `SampleShop 1.4.1 · bukkit (plugin.yml)` | Name, version, platform and descriptor; one line per descriptor. |
| `sha256 … · 6 classes · obfuscation 0/100` | Start of the file's SHA-256, class count, obfuscation score. |
| `call graph from …: 10 of 12 methods reached` | The first entry point, "and N more" if there are others, and how much of the plugin's own code they reach. Absent when there's no graph. |
| `the plugin also loads code at runtime…` | The `dynamic` flag: code marked not called may still run. |
| `PARTIAL REPORT: …` | A scan limit was hit. Absent here. |
| Finding blocks | Severity, title, rule id; the why line; up to 3 places; "not called" when it applies. Findings in code nothing calls come after all others, whatever their severity. |
| `Warnings:` | One line per warning: code, location, message. Absent here. |
| `2 high · 4 notable · 5 info · …` | Counts, and how many findings are in code nothing calls. |
| The last line | The disclaimer, once, after all the jars in a scan. |

The website orders findings a little differently: Info findings get their own fold, and not-called findings go to the end of the High and Notable list and of that fold. The command line moves them to the end of the whole report.

## Where a finding is

| Location | What it is |
|---|---|
| `com/example/Main.onEnable` | A method in the plugin's own classes. |
| `com/example/Main` | A whole class, for class-level evidence such as an event handler. |
| `libs/helper.jar!/com/example/Net.send` | A class inside a nested jar. |
| `com/example/Main!embedded-1!/payload/Loader` | Code decoded from a string in `com/example/Main`. |
| `plugin.yml` | A resource file in the jar. |
| `(jar)` | The jar as a whole, as for the obfuscation score. |

## The value after the title

Rules about a specific thing report one finding per value, each with every place it appears.

| Rule | Value |
|---|---|
| `str.url` | The host, like `api.example.invalid`. |
| `str.ip` | The address, with its port when it has one. |
| `str.discord-webhook` | The webhook id. The token is cut to its first 4 characters, so you can share a report. |
| `str.telegram-bot` | The bot id, its token cut the same way. |
| `str.login-file` | The matched path text, like `.ssh/id_`. |
| `blob.embedded-code` | The SHA-256 of the decoded bytes. |
| `res.type-mismatch`, `jar.nested`, `jar.unsafe-path` | The entry's path. |
| `meta.missing-main` | The missing class name. |

## Partial or unreadable

- **Unreadable:** the file isn't a zip, or its central directory can't be read. Nothing in it was checked, so it has no findings, only a `zip.unreadable` warning. The command line exits with 1.
- **Partial:** a size, count or depth limit was hit, or entries were encrypted or damaged. Everything found up to that point is listed, but part of the jar wasn't checked. A partial report is not a clean one. The limits are in [Limits](limits.md).

## Warning codes

<!-- include: docs/architecture.md#warning-codes -->

## How to judge a finding

1. **Does it fit what the plugin says it does?** An updater connects to the internet, a rewards plugin runs console commands, a backup plugin writes zip files.
2. **Do you know the hosts?** Read the `str.url` and `str.ip` values. A plugin's own site, its update API, bStats and Maven repositories are ordinary.
3. **Is it called?** A notable finding in an unused part of a bundled library is less pressing than one in the plugin's own command handler. It can still run if the plugin loads code at runtime.
4. **Was it in the last version?** [Compare](compare.md) the version you run with the update. A capability that's new in an update, especially with a new author or website, is the moment to look closer.

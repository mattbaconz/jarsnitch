# Check against official releases

A copy of a plugin can differ from the one its author published: re-uploaded on another site with code added, or changed on your server by something that infects jars. A check finds the file the project published on Modrinth or Hangar, tells you whether yours is that file, and if it isn't, shows class by class what's different. It also finds the latest release and shows what the update adds.

This is the one part of jarsnitch that uses the network, and only when you ask for it.

## What you get for each jar

| Result | Meaning |
|---|---|
| **Official file** | These exact bytes are a file the project published on Modrinth or Hangar. |
| **Same code** | Not the same file, but it runs the same code as the official file of this version: every class and native library is the same, and so are the main class and the libraries the server downloads at startup. Some other file differs, or only the zip around the files does, as when a jar is re-packed. A re-signed jar also has a different `MANIFEST.MF`. When the plugin descriptor, the manifest or a service list differs, the result says so: the server reads those to set commands, permissions and what loads. |
| **Code differs** | The official file of the same plugin and version has different code: a class, a native library, the main class or a startup library differs, or a scan limit kept part of a jar from being read. You see what this jar adds and every class that changed. |
| **Not found** | Neither site has a file with these bytes, or a file of this plugin and version to compare with. When the project named like the plugin has a file of this version that isn't the same plugin (another name, or another main class and most classes different), the result names it, and you can compare the two with `jarsnitch diff`. |

Then, when there's a newer release: its version and date, and a comparison of your jar with it, the same as [comparing two versions](compare.md).

**Code differs** is what a jar with injected code looks like. A development build, your own patched build or a fork that kept the version number looks the same, so read what changed before you decide anything.

**Official** means the project published that file. It says nothing about what the code does: an author's account can be taken over, and then an official release can contain anything. Scan it, or compare the update before you install it. Anyone can also publish a copy of a plugin as a project of their own, so when the project a jar is matched with isn't named like the plugin, the result says so.

**Not found** is normal for paid plugins (SpigotMC and BuiltByBit give each buyer their own copy), private and custom builds, and plugins published only on SpigotMC, GitHub or a CI server.

## On the website

1. Scan one jar, several, or a dropped plugins folder.
2. Under **Official release** (**Official releases** for several jars), read what the check sends, then press **Check against official releases** (**Check all against official releases**).
3. Each jar gets its result. For a jar whose code differs, **What this jar adds to the official file** opens the comparison with that file. **What the update changes** opens the comparison with the latest release.

Nothing is sent until you press the button. The website looks jars up on Hangar by their bytes only, and checks updates on Modrinth only; the command line does both on both sites.

## On the command line

```bash
jarsnitch check plugins/
```

```bash
jarsnitch check plugins/ --mc 1.21.4
```

`check` takes jars and folders like `scan`. Options:

- **`--mc <version>`** looks only for updates that list this Minecraft version, on Modrinth and on Hangar. Without it, the latest release for your server software counts, whatever Minecraft versions it lists.
- **`--no-updates`** skips the update lookup and its downloads.
- **`--json`** prints an array with one object per jar; see [JSON format](json.md#a-check).

Before it sends anything, `check` says on standard error what it's about to send and to which sites. Here it is on a folder with an official jar, a re-packed copy of another and the website's made-up sample plugin:

```out $ jarsnitch check plugins/
== SampleShop-1.4.1.jar ==
SampleShop 1.4.1
Not found on Modrinth or Hangar: no file with these bytes, and no official SampleShop 1.4.1 to compare with.
  Paid, private and custom builds can't be checked this way.

== coreprotect.jar ==
CoreProtect 24.0
Official file: the same bytes as CoreProtect 24.0 on Modrinth (https://modrinth.com/project/coreprotect)
Update: 24.1, 2026-09-24 (https://modrinth.com/project/coreprotect/version/3sehX6Sg)
  adds no notable or High findings
  code: 23 classes changed, 6 added, 0 removed

== griefprevention-rezipped.jar ==
GriefPrevention 16.18.7
Same code as GriefPrevention 16.18.7 on Modrinth (https://modrinth.com/project/griefprevention), but not the same file.
  Every file is the same; only the zip around them differs.
Update: 16.18.8, 2026-11-01 (https://modrinth.com/project/griefprevention/version/MtwPHMAS)
  adds no notable or High findings
  code: 17 classes changed, 3 added, 0 removed

3 jars · 1 official · 1 same code · 0 code differs · 1 not found · 2 updates available
Official means the project published that file; it isn't a check of what the code does. jarsnitch scan shows that.
```

› real output: jarsnitch 0.4.0 · exit code 0 · Modrinth as of 15 Nov 2026

A jar whose code differs prints the official file's address, what the jar adds to it, and the class-by-class comparison:

```text
== MyPlugin-2.0.0.jar ==
MyPlugin 2.0.0
CODE DIFFERS from MyPlugin 2.0.0 on Modrinth (https://modrinth.com/project/myplugin)
  official file: https://cdn.modrinth.com/data/…/MyPlugin-2.0.0.jar

This jar ADDS:

HIGH     Can download and run code from the internet   [corr.download-and-run]
         The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.
         at com/example/myplugin/MyPlugin.added$0

NOTABLE  Can load new code while running   [code.load]
         Code that isn't in this jar can be added later, and jarsnitch can only judge what's in the file.
         at com/example/myplugin/MyPlugin.added$0

INFO     Contains web addresses: payload.example.invalid   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at com/example/myplugin/MyPlugin.added$0

Code: 1 class changed, 0 added, 0 removed, 41 unchanged

HIGH     Changed class: com/example/myplugin/MyPlugin   [code.load, corr.download-and-run]
         + method added$0()V
         ~ method onEnable()V
         now calls: com/example/myplugin/MyPlugin.added$0, java/net/URL.<init>, java/net/URLClassLoader.<init>, java/net/URLClassLoader.loadClass
```

› example: MyPlugin and its addresses are made up. The findings and class changes are the real output for the `inject-v1` and `inject-v2` test fixtures, where one method was added to the main class and called at the start of `onEnable`.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Every jar was checked; none has code that differs from its official file, and no jar or update adds a High finding. |
| 1 | A jar couldn't be read, a request failed, an update comparison was incomplete, or there was nothing to check. |
| 2 | A jar's code differs from its official file, or a jar adds a High finding to it, or an update adds one. |

2 wins over 1. A jar that isn't found doesn't change the exit code.

An update that cannot be opened as a jar is reported as an error. If some contents cannot be scanned, the comparison is marked incomplete and keeps the findings it did observe, including High findings. It cannot establish that the update adds no notable or High findings.

## How a jar is matched

1. **Its exact bytes.** The SHA-512 hashes of all the jars go to Modrinth in one request. Plugins Modrinth doesn't have are looked up on Hangar by SHA-256, one request each.
2. **The version it says it is.** For a jar with no match, the plugin name and version come from its `plugin.yml` (or `paper-plugin.yml`, `bungee.yml`, `velocity-plugin.json`, or a mod's descriptor). jarsnitch looks for the Modrinth project by that name, then through Modrinth's search, and lists the project's releases for your server software. A release counts when its version number is the jar's, allowing a leading `v` and a suffix such as `-bukkit`, but not a pre-release one such as `-SNAPSHOT` or `-beta.1`. Then it tries the project of that name on Hangar.
3. **The same plugin.** Up to 3 official files of that version are downloaded from Modrinth, and, when none of them is the same plugin, up to 3 from Hangar. Each is checked against the hash the site lists. One counts only when its descriptor has the same plugin name, and the same main class or at least half of the same classes, so a different plugin with the same name and version number doesn't count. Of those, the closest one is compared with your jar. A download that fails or doesn't match its hash is skipped and reported, so the exit code is 1 unless something makes it 2.
4. **Updates.** Modrinth's latest release for your server software (only the release channel, and only for `--mc` when you give it). Projects that publish separate builds (Bukkit and Paper, or one per Minecraft version) keep your build: the file name without its version has to match. Another build of the version you have isn't an update, even when it was uploaded later. When a project renamed its files, the newest release's only jar counts, unless older releases, or other builds of your version, had that file too. On Hangar, the project's latest release, or with `--mc` the newest release for that Minecraft version, when the jar's own version is on Hangar and the release doesn't hold your file or the official file it matched. Only a release published after the one you have counts. The update is downloaded, checked against its hash and compared with your jar. A download that fails is an error; a Hangar release with no file for your server software is listed as not compared.

Renamed files must also retain explicit platform words such as Bukkit or Paper and numeric Minecraft/build markers outside the release version. A change, addition or removal of those markers is not enough evidence to recommend a different build.

Classes are compared method by method, on the instructions, the constants they use, their try/catch handlers and their annotations, so a rebuild that only renumbers the constant pool, or a jar re-packed with other compression, isn't a change. See [What counts as a change](compare.md#what-counts-as-a-change). Comparisons with an update also show ordinary rebuild noise, like renumbered lambdas; read what it adds first.

## What it sends

- To `api.modrinth.com`: the SHA-512 hash of every jar, the plugin names and versions in searches and project lookups, and the hashes of matched official files for the update lookup.
- To `hangar.papermc.io`: the SHA-256 hash of jars Modrinth doesn't have, and plugin names and versions.
- Downloads of official files and updates from `cdn.modrinth.com` and Hangar's CDN.
- The command line tool sends the header `User-Agent: mattbaconz/jarsnitch/<version> (+https://github.com/mattbaconz/jarsnitch)`, as Modrinth asks. Browsers send their own.

Your jars are never sent. There's no account, key or cookie, and nothing is stored. Requests are spaced to stay under Modrinth's and Hangar's rate limits. More in [Privacy](privacy.md).

## What it can't do

- **Other sites.** SpigotMC, BuiltByBit and CurseForge publish no file hashes jarsnitch can use without an account, and GitHub releases aren't looked up.
- **Paid plugins.** Each buyer gets a different file, so there's no official copy to match.
- **Wrong version numbers.** A jar that claims a version the project never published, or a project that numbers releases differently from its `plugin.yml`, ends up **Not found**.
- **Builds that differ by design.** Modrinth sometimes holds only some builds of a plugin, and a site's copy and the author's CI build can differ. The check reports what it compared; it can't tell the author's own build from one someone else changed.
- **Stale mirrors.** A project that stopped uploading to one site makes the latest release there look older than it is.

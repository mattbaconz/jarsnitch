# Compare two versions

A plugin you already trust can change in an update: a new owner, a compromised author account, a "fixed" re-upload from an unofficial site. A comparison reads both versions and leads with what the new one adds.

## How a comparison works

<!-- diagram: diff-flow -->

Findings are matched by rule and value (a host, an IP address, a webhook id, a path), not by where they sit in the code. Code that only moved inside the jar doesn't make a new finding, and a capability the old version already had counts as unchanged even if it now appears in more places. What's new in a moved class still shows: a new class that carries notable or High evidence is listed under Adds.

## On the website

1. Open the **Compare two versions** tab (or press **2** while the tabs or a drop zone have focus).
2. Drop the version you run on **Old** and the new one on **New**. **Swap old and new** fixes a mix-up.
3. The comparison opens as soon as both are in.

You can also drop both jars at once anywhere on the page. Two jars whose names share a stem before the version, like `Foo-1.9.jar` and `foo_2.0.jar`, are compared automatically, and so are any two jars dropped while the Compare tab is open. The older name goes on the left: version numbers are sorted as numbers, so `Foo-1.9.jar` comes before `Foo-1.10.jar`. A dropped folder is always scanned, never compared.

**Try a sample update** compares SampleShop 1.4.0 with 1.4.1, a made-up update that names a new author and adds a downloader and a hidden operator command.

<!-- gif: site-compare | The jarsnitch page comparing the sample update SampleShop 1.4.0 to 1.4.1: the update adds 2 high, 3 notable and 2 info findings, the first High finding's locations open, and the Changes section shows a new author. -->

## On the command line

```bash
jarsnitch diff MyPlugin-1.4.jar MyPlugin-1.5.jar
```

<!-- gif: cli-diff | jarsnitch diff SampleShop-1.4.0.jar SampleShop-1.4.1.jar run in a terminal: the update adds two High findings, a way to download and run code and a possible hidden command, with their new classes and notable findings, and names a new author and website. -->

The old version comes first. Here is the sample update:

```out $ jarsnitch diff SampleShop-1.4.0.jar SampleShop-1.4.1.jar
== SampleShop-1.4.0.jar -> SampleShop-1.4.1.jar ==
SampleShop 1.4.0 -> 1.4.1
sha256 62903a86fa08 -> 699600d2ac6a · obfuscation 0 -> 0

This update ADDS:

HIGH     Can download and run code from the internet   [corr.download-and-run]
         The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.
         at example/sampleshop/AddonLoader.load

HIGH     Possible hidden command   [corr.hidden-command]
         Typing a specific word in chat could give someone operator or run console commands, without that command appearing in the plugin's command list.
         at example/sampleshop/ChatListener

HIGH     New class: example/sampleshop/AddonLoader   [code.load, corr.download-and-run, fs.write-archive]

HIGH     New class: example/sampleshop/ChatListener   [corr.hidden-command, mc.set-op]

NOTABLE  Can load new code while running   [code.load]
         Code that isn't in this jar can be added later, and jarsnitch can only judge what's in the file.
         at example/sampleshop/AddonLoader.load

NOTABLE  Can write .jar or .class files   [fs.write-archive]
         Writing jar or class files is how code installs or changes plugins, including other plugins.
         at example/sampleshop/AddonLoader.load

NOTABLE  Can give players operator   [mc.set-op]
         An operator can run every command on the server.
         at example/sampleshop/ChatListener.onChat

INFO     Contains web addresses: cdn.example.invalid   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at example/sampleshop/AddonLoader.load

INFO     Contains web addresses: shop-plugins.example.net   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at plugin.yml

This update REMOVES:

INFO     Contains web addresses: sampleshop.example.com   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at plugin.yml

Changes:
  plugin.yml authors: ExampleDev -> ExampleNewOwner
  plugin.yml version: 1.4.0 -> 1.4.1
  plugin.yml website: https://sampleshop.example.com -> https://shop-plugins.example.net
A different author or website can mean the plugin has a new owner.

Unchanged: mc.console-command, net.http, net.socket, str.url

This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.
```

› real output: jarsnitch 0.2.1 on the site's sample jars · exit code 2

- The exit code is **2** when the update adds a High finding, **0** otherwise.
- If either file can't be read, or isn't a zip, `diff` prints an error to standard error and exits with **1**. Nothing goes to standard output, not even with `--json`, so a script never mistakes a failed comparison for an empty one.
- `--json` prints one object; see [JSON format](json.md#a-comparison).

## Reading a comparison

### Adds

Everything only the new version has: findings, **new classes** that carry notable or High evidence (with the rules they triggered), commands, permissions, plugin dependencies (required and optional), libraries the server downloads at startup, and bundled libraries. Items are sorted by severity; within one severity, findings come first, then classes, then the rest, which count as info. This is the part to read.

An added finding in code the plugin's entry points don't call is marked "not called". The website also moves those findings to the end of each run of findings; the command line keeps the severity order.

In the sample, the update adds a class that downloads a jar and loads it, and a chat listener that can give a player operator. Each is a High finding on its own line, followed by the new classes that hold them.

### Removes

The same kinds, gone from the new version. In the sample, the old website address.

### Changes

Values that changed under the same key: bundled and downloaded library versions, nested jar hashes, and the plugin's name, version, main class, authors and website. When the authors or the website differ, the comparison adds that this can mean the plugin has a new owner. Authors are compared as a sorted list, so a new order isn't a change.

### Unchanged

Findings both versions have, listed by rule id with a count when a rule has several values (`str.url (2)`).

## What counts as a change

<!-- include: docs/architecture.md#matching -->

## Notes a comparison can add

- **New owner:** under Changes, when the authors or the website differ.
- **`diff.classes-renamed`:** fewer than half of the old class names still exist and either version scores 25 or more for obfuscation. The jar was probably re-obfuscated, so new and removed classes are left out for this pair. Findings are still compared.
- **`diff.partial`:** a scan limit was hit on either side, so part of a jar wasn't checked.

## Use it before every update

A script that refuses an update with new High findings. Exit code 1 (a file couldn't be read) stops it too.

```bash
#!/usr/bin/env bash
# usage: check-update.sh plugins/MyPlugin.jar downloads/MyPlugin-new.jar
jarsnitch diff "$1" "$2"
case $? in
  0) echo "No new High findings. Read the ADDS section, then install." ;;
  2) echo "The update adds a High finding. Not installing."; exit 2 ;;
  *) echo "Couldn't compare the two files. Not installing."; exit 1 ;;
esac
```

```powershell
# usage: .\check-update.ps1 plugins\MyPlugin.jar downloads\MyPlugin-new.jar
jarsnitch diff $args[0] $args[1]
switch ($LASTEXITCODE) {
  0 { "No new High findings. Read the ADDS section, then install." }
  2 { "The update adds a High finding. Not installing."; exit 2 }
  default { "Couldn't compare the two files. Not installing."; exit 1 }
}
```

## What a comparison can't see

It compares what's in the two files. Code either version downloads or builds at runtime is invisible to both scans, and the [limits of a single scan](limits.md) apply to each side.

# FAQ

Short answers, with links to the longer ones.

## Will jarsnitch tell me whether a plugin is malware?

No. jarsnitch never calls a plugin malware, safe or clean. It reports what the code is able to do, where, and why that matters, and leaves the judgement to you. A legitimate updater and a harmful downloader can use exactly the same calls, so a verdict from the code alone would be a guess.

## Why does a popular plugin have notable findings?

Because most capabilities have ordinary uses. Of Modrinth's 100 most-downloaded plugins, 93 contain web addresses, 70 can connect to websites, 21 can load code at runtime and 14 can run operating-system commands. Each rule lists its common legitimate uses. The question is whether a finding fits what the plugin says it does.

› Modrinth top 100, latest releases · jarsnitch 0.2.1 · 13 Nov 2026 · [Benchmarks](../benchmarks.md#what-the-top-100-can-do)

## What does "not called" mean? Can that code still run?

Nothing the plugin starts from (its main class, the methods the server calls on it, event handlers, classes named in service files or the manifest) reaches that code. It usually belongs to an unused part of a bundled library. It can still run if the plugin reaches it through reflection with a computed name, or loads code at runtime; the report says when the plugin does that. Severity never changes. See [The call graph](call-graph.md).

## Does jarsnitch run the plugin?

No. It reads the jar's bytes like any other file and parses the class files itself. No Java runs, on the website or on the command line.

## Which plugins does it understand?

Bukkit, Spigot and Paper (`plugin.yml`, `paper-plugin.yml`, including Paper's `bootstrapper` and `loader`), Folia (the `folia-supported` flag), BungeeCord (`bungee.yml`) and Velocity (`velocity-plugin.json`). Fabric, Quilt, Forge and NeoForge mods get their name and version and the same bytecode rules, without a call graph.

## Does it use signatures for known threats?

No. It ships no signatures and won't name a family of known code. It looks for capabilities and combinations, which also covers code it has never seen. If known-bad hashes or hosts are ever added, each will come with a public source.

## My plugin is obfuscated. What then?

The report includes an obfuscation score, and a finding at 25 or more. Obfuscation is common in paid plugins and isn't a problem by itself, but it makes code harder to read for people and for jarsnitch, so findings may be incomplete. Text the obfuscator encrypts stays hidden; text it only encodes as a constant (byte arrays, base64, hex) is decoded.

## Why does it mention a new owner?

When an update changes a plugin's declared authors or website, the comparison says this can mean the plugin has a new owner. Plugins change hands, and a new owner can ship code the old one never would. It's a reason to read what the update adds, nothing more.

## It says "not a readable jar"?

The file isn't a zip, or its central directory is damaged, so nothing in it was checked. Download it again from the original page.

## What is a partial report?

A size or count limit was hit, or some entries were encrypted or damaged, so part of the jar wasn't checked. Everything found is listed, but a partial report is not a clean one. See [Limits](limits.md).

## Can I use it in CI?

Yes. `jarsnitch scan plugins/` exits with 2 when any jar has a High finding and with 1 when something couldn't be read. [Command line](cli.md#recipes) has a GitHub Actions step.

## How is it different from PluginGuard and PluginScan?

<!-- include: README.md#similar-tools -->

## Is SampleShop a real plugin?

No. SampleShop is made up for the website's samples and these docs. Its 1.4.1 "update" was written to show what a comparison catches, and its code never runs: the downloader points at `.invalid` hosts.

## I think a finding is wrong.

Open an issue on [GitHub](https://github.com/mattbaconz/jarsnitch/issues) with the plugin's name, version and the finding. Rules are tuned against real plugins with the false-positive bench in `scripts/fp-bench`.

## I found a bug in jarsnitch itself.

A jar that crashes or hangs the scanner, gets past its limits, or makes the website do anything beyond the analysis is a security bug. Report it privately; see [SECURITY.md](../../SECURITY.md).

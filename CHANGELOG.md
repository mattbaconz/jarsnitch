# Changelog

## 0.4.0 - 2026-11-16

- [added] `jarsnitch check <jar|folder>` looks each jar up on Modrinth and Hangar and says whether it's the file the project published, the same code in a different file, or code that differs from the official file of its version, and then shows what it adds. It also finds the latest release for your server software (`--mc` for one Minecraft version, `--no-updates` to skip) and compares it with your jar. Exit code 2 when a jar's code differs from the official file, the jar adds a High finding to it, or the update adds one. It's the only command that uses the network: it sends hashes, plugin names and versions, never the jars. See [Check against official releases](docs/guide/check.md).
- [added] On the website, **Check against official releases** does the same for one jar or several. Nothing is sent until you press it. Updates are checked on Modrinth.
- [added] Comparisons compare code: every class in both jars, method by method, with the methods and fields added, removed or changed and the calls each class now makes, so code added to a class that already existed shows. Try/catch handlers and annotations count. A rebuild that only reorders the constant pool, or a jar re-zipped or signed again, isn't a change. Two entries with the same class name are both compared. It's `code` and `same_code` in the JSON, a `Code:` part on the command line and a **Code** section on the website.
- [added] `diff.same-version`, a note when both jars give the same version but their code differs, and `diff.native-changed`, when a native library differs.
- [fixed] A scan or comparison can't be made to take minutes by code that names a long string, or an `invokedynamic` with a long bootstrap list, over and over. Fingerprints hash each long string once per class; the scan walks each bootstrap list once per method and stops after 1 GiB of names and strings (`Limits::max_code_work` in `jarsnitch-core`), with a `class.limit` warning that marks the report partial. Real plugins use under a tenth of that.
- [changed] The website's Content Security Policy allows `api.modrinth.com`, `cdn.modrinth.com` and `hangar.papermc.io`, for the check only.
- [fixed] Renamed update files retain explicit server-platform and Minecraft build markers. Unreadable downloads fail the check, incomplete update comparisons retain their findings and report an error, and malformed Hangar project responses no longer appear as successful lookups.

## 0.3.0 - 2026-11-14

- [added] Call paths. Each finding the plugin's entry points reach carries one call path from an entry point to the code, with how each step leads to the next: entry, call, callback, static initializer or reflection. In the JSON it's `path` on each finding, and each evidence entry has its own `reached`. Severity, counts and exit codes don't change.
- [added] On the website, each card for a single finding has a trail: the call path from an entry point to the call or text that matched, or, without one, the place in the jar, with a "no caller" break when nothing calls the code. Findings of one rule that differ only in value, such as hosts, share a card that lists the values instead. Each report also has a "Where in the jar" section with a call graph meter and a tree of the packages, classes and files with findings, one mark per rule: solid when reached, hollow when not called, a circle for what the call graph doesn't cover.
- [fixed] A finding with reached code is now marked reached even when another of its places isn't code the call graph covers, such as a class in a nested jar. Before, it got `reached: null` and didn't move with the reached findings.
- [fixed] Which methods a virtual call reaches no longer depends on whether the scan meets the call or the object's creation first. A call on a subclass now always reaches the superclass method it may land on, and a call through an interface reaches a method the created class inherits from a superclass, which 0.2.1 missed when the object was created before the call.
- [changed] The website's main page is now just the tool. The longer sections that were on it, such as how a finding is made, the top 100 numbers and the command line, are in the docs.

## 0.2.1 - 2026-11-14

- [fixed] Descriptors written as JSON, which YAML allows (some build tools write `paper-plugin.yml` that way), are read properly. 0.2.0 read their values with the quotes and commas left in, so it reported a missing main class and built no call graph.
- [changed] A new website design, with a logo, a sample report and CLI output on the first screen, and a "Reading this report" legend beside every report.
- [added] Docs pages on the website at `/docs/`: getting started, scanning, comparing, reading a report, the call graph, every rule, the command line, the JSON format, limits, privacy and a FAQ, with diagrams and GIFs. The sources are in `docs/guide/`.
- [added] Benchmarks on Modrinth's 100 most-downloaded plugins, in [docs/benchmarks.md](docs/benchmarks.md), with the scripts in `scripts/bench/`.

## 0.2.0 - 2026-11-12

- [added] Reachability: each finding says whether the plugin's entry points can call the code it was found in (`reached` in the JSON, with a `reachability` summary). The CLI and the web page list findings in code nothing calls last and mark them. Severity and exit codes don't change.
- [added] `corr.download-and-run`, `corr.command-and-network` and `corr.login-files-and-network` also fire when one method calls parts that sit in other classes.
- [added] The web page scans several jars at once, or every jar directly in a dropped plugins folder, with one row per plugin, most High findings first.
- [added] "Try a sample plugin" and "Try a sample update" on the web page: SampleShop, a made-up plugin whose 1.4.1 update names a new author and adds a downloader and a hidden operator command.
- [added] `diff` lists new and removed plugin dependencies, required and optional, and changes to a plugin's authors and website, with a note that a different author or website can mean a new owner. Reports carry `authors`, `website`, `dependencies` and `optional_dependencies` for Bukkit, Paper, BungeeCord and Velocity plugins, and the web page shows them under "About this jar".
- [fixed] Descriptor list items written at the same indent as their key (`libraries:` followed by unindented `- item` lines, which YAML allows) were missed.
- [fixed] The web page's "About this jar" section showed `[object HTMLDListElement]` instead of the jar's details.

## 0.1.0 - 2026-11-09

- [added] `jarsnitch scan` and `jarsnitch diff`, with 33 rules for system commands, runtime code loading, network access, operator and console control, hidden text and code, login files, nested jars, plugin metadata and obfuscation. See [docs/rules.md](docs/rules.md).
- [added] A web page that scans and compares jars in the browser. Files never leave your machine.

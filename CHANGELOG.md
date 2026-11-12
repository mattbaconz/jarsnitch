# Changelog

## Unreleased

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

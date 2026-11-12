# Changelog

## Unreleased

- [added] Reachability: each finding says whether the plugin's entry points can call the code it was found in (`reached` in the JSON, with a `reachability` summary). The CLI and the web page list findings in code nothing calls last and mark them. Severity and exit codes don't change.
- [added] `corr.download-and-run`, `corr.command-and-network` and `corr.login-files-and-network` also fire when one method calls parts that sit in other classes.

## 0.1.0 - 2026-11-09

- [added] `jarsnitch scan` and `jarsnitch diff`, with 33 rules for system commands, runtime code loading, network access, operator and console control, hidden text and code, login files, nested jars, plugin metadata and obfuscation. See [docs/rules.md](docs/rules.md).
- [added] A web page that scans and compares jars in the browser. Files never leave your machine.

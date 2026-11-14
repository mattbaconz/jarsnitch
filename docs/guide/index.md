# Docs

> **"Can" means the code is in the jar, not that it runs.**
>
> jarsnitch reads a plugin's bytecode and lists what that code is able to do, where it sits and why it matters. It never loads or runs the plugin, and it never calls a plugin malware, safe or clean. Whether a finding fits the plugin is your call.

These pages cover the website at <https://jarsnitch.mbaconz.workers.dev> and the `jarsnitch` command line tool. Both run the same engine and report the same findings.

## Where to start

- **I have a jar.** Drop it on the website or run `jarsnitch scan MyPlugin.jar`. Start with [Scan a plugin](scan.md), then [Read a report](report.md).
- **I'm about to update a plugin.** Compare the version you run with the new one and read what the update adds. Start with [Compare two versions](compare.md).
- **I check plugins in CI or an update script.** Exit code 2 means a High finding, or an update that adds one. Start with [Command line](cli.md).

## Severities

<!-- include: docs/rules.md#severities -->

Severity comes from the rule alone. Whether the plugin's entry points call the code never changes it; see [The call graph](call-graph.md).

## Most findings have ordinary explanations

Most capabilities have everyday uses, and every rule lists the common ones. Paid plugins often obfuscate their code and contact a license server. Reward plugins run console commands from their config. Dependency loaders download libraries at startup. Ask whether each finding fits what the plugin says it does, and whether the hosts in it are ones you'd expect.

## Reading a comparison

A comparison of two versions of one plugin leads with what the new version adds. It has four parts:

- **Adds:** findings, classes with notable or High evidence, commands, permissions, dependencies and libraries that only the new version has. Start here.
- **Removes:** the same kinds, gone from the new version.
- **Changes:** library versions, nested jar hashes, and the plugin's name, version, main class, authors and website. A different author or website can mean the plugin has a new owner.
- **Unchanged:** findings both versions have. They're matched by rule and value, so code that moved, or a capability that now appears in more places, stays here.

More in [Compare two versions](compare.md).

## All pages

<!-- pages -->

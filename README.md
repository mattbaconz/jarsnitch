# jarsnitch

See what a Minecraft plugin can do before you install it.

> **Status: pre-release.** `jarsnitch scan`, `jarsnitch diff` and the website work. Nothing is released or hosted yet; build from source below.

jarsnitch reads a plugin `.jar` (Bukkit, Spigot, Paper, Folia, Velocity, BungeeCord) and tells you in plain English what its code is able to do: run commands on your machine, download and run new code, give players operator, send data to a Discord webhook. Each finding says where in the code it was found and why it matters.

It reads the bytecode. It never loads or runs the plugin.

## Two ways to use it

**Website.** Drop one jar to scan it, or two to compare versions. The analysis runs in your browser (WebAssembly). Your files never leave your machine: no upload, no backend, no telemetry. Once the page has loaded it needs no network at all.

**Command line.**

```bash
jarsnitch scan MyPlugin.jar
```

```bash
jarsnitch scan plugins/
```

```bash
jarsnitch diff MyPlugin-1.4.jar MyPlugin-1.5.jar
```

## Check an update before you install it

A plugin you already trust can change in an update: a new owner, a compromised author account, a "fixed" re-upload from an unofficial site. `diff` compares two versions and leads with what the new one adds.

```text
== MyPlugin-1.0.jar -> MyPlugin-1.1.jar ==
Fixture-diff 1.0.0 -> 1.1.0
sha256 95853045dab4 -> b9701ed31629 · obfuscation 0 -> 0

This update ADDS:

NOTABLE  Can load new code while running   [code.load]
         Code that isn't in this jar can be added later, and jarsnitch can only judge what's in the file.
         at fixture/Addons.load

NOTABLE  Can send data to a Discord webhook: 000000000000000000   [str.discord-webhook]
         A webhook built into the code posts to a Discord channel the author controls, not you.
         at fixture/Main.sendStats

This update REMOVES: nothing

Changes:
  plugin.yml version: 1.0.0 -> 1.1.0

Unchanged: net.http, str.url (2)

This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.
```

*(Real output for the test fixtures `fixtures/jars/diff-v1.jar` and `diff-v2.jar`, copied as `MyPlugin-1.0.jar` and `MyPlugin-1.1.jar`.)*

Findings are matched by rule and value (host, webhook id, path), not by where they sit, so code that only moved doesn't show up. A capability the old version already had counts as unchanged even if it now appears in more places.

## What a finding tells you

- **Severity:** info, notable or high. See [severities](docs/rules.md#severities).
- **Title** in plain English, for example "Can give players operator".
- **Why it matters**, in one line.
- **Typical legitimate uses**, because most capabilities have them.
- **Where:** class and method, with nested jars shown as `libs/helper.jar!/com/example/Net`.

High is used for combinations that give a jar a lot of control (for example network access plus runtime code loading) and for bundled libraries with known security bugs.

## What it looks for

| Area | Examples |
|---|---|
| Your machine | Running OS commands, loading native code, Java agents, instant shutdown |
| New code | Runtime class loading, code hidden in encoded strings, writing jar files |
| Your server | Giving operator, running console commands, possible hidden chat commands |
| Network | Web and raw socket connections, hardcoded hosts and IPs, Discord and Telegram webhooks |
| The jar itself | Nested jars, files disguised as another type, unsafe paths, obfuscation score, plugin.yml problems, bundled log4j with known bugs |

Full list with ids, severities and exactly what triggers each: [docs/rules.md](docs/rules.md).

## How to read a report

- **"Can" means the code is in the jar, not that it runs.** jarsnitch doesn't trace whether a capability is ever used. A bundled library may carry features the plugin never calls.
- **Most findings have ordinary explanations.** Paid plugins often obfuscate their code and contact a license server. Reward plugins run console commands from their config. Dependency loaders download libraries at startup. Ask whether a finding fits what the plugin says it does.
- **jarsnitch never calls a plugin malware, safe or clean.** It reports facts and leaves the judgement to you.

Every report ends with:

> This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.

## What it can't catch

- **Code that isn't in the jar.** Anything downloaded or generated at runtime is invisible. You'll see that the jar *can* load code, not what it loads.
- **Names built at runtime.** Reflection is resolved only when class and method names are plain string literals. Names assembled, decrypted or downloaded at runtime are not.
- **Encrypted payloads.** Base64 and hex are decoded. XOR, AES or custom encoding is not; it may only show up as a higher obfuscation score.
- **Intent.** A legitimate updater and a malicious dropper can use exactly the same calls.
- **Combinations spread over classes.** The High findings for network access plus code loading or OS commands need both parts in one class. Split across classes, each part is still listed on its own.
- **Reachability.** No call-graph analysis: a finding in dead code or an unused library counts the same as one in `onEnable`.
- **Known malware by name.** v1 ships no malware signatures. It won't tell you "this is family X".
- **Non-Java code in resources.** Scripts (Skript, JavaScript) and native libraries inside the jar are not analyzed, though disguised ones are flagged.
- **Mods.** Fabric, Quilt, Forge and NeoForge jars get their name and version read and the same bytecode rules, but no mod-specific checks.
- **Exotic archive tricks.** jarsnitch reads the zip central directory the way the JVM does. Archives built to look different to different readers may still fool it.
- **Very large or hostile archives.** Scans stop at size and count limits. The report is then marked partial, and a partial report is not a clean one.

## Command line reference

```text
jarsnitch scan <jar|folder>... [--json]
jarsnitch diff <old.jar> <new.jar> [--json]
```

- `scan` on a folder scans every `.jar` directly inside it (not subfolders).
- `--json` prints the full report. `scan` prints a JSON array with one report per jar; `diff` prints one object. Format: [docs/architecture.md](docs/architecture.md#report-format).

Exit codes:

| Code | `scan` | `diff` |
|---|---|---|
| 0 | No High findings | The update adds no High findings |
| 1 | A file couldn't be read or isn't a zip (and no High findings) | Either file couldn't be read |
| 2 | At least one High finding | The update adds at least one High finding |

Use exit code 2 to fail a CI job or an update script.

## Install

Not released yet. From source, with Rust stable:

```bash
cargo install --path crates/cli
```

The website, built locally (needs the wasm32 target: `rustup target add wasm32-unknown-unknown`):

```bash
./scripts/build-web.sh
```

```bash
python -m http.server 8080 -d web
```

Then open http://localhost:8080. On Windows, run `scripts\build-web.ps1` instead of the first command. `web/` is a plain static site; any static host works. Send the headers in `web/_headers` if your host supports them.

## Similar tools

- [PluginGuard](https://github.com/denfry/PluginGuard): server-side analyzer you upload jars to, with an optional sandbox, malware signatures and install recommendations.
- [PluginScan](https://github.com/Rikonardo/PluginScan): web and JVM CLI scanner with community-written checks.

jarsnitch focuses on comparing versions, runs entirely on your machine, and reports capabilities instead of verdicts.

## Docs

- [Rules](docs/rules.md): every rule, what triggers it, typical legitimate uses
- [Architecture](docs/architecture.md): crates, report format, diff, limits, web app
- [Contributing](CONTRIBUTING.md): adding a rule, fixtures, tests

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

# Contributing

Thanks for helping. Most contributions are new rules, better rule copy, or false-positive fixes.

## Setup

- Rust stable
- JDK 17 or newer, only to rebuild fixture jars
- For the web page: `rustup target add wasm32-unknown-unknown` and `wasm-bindgen-cli` at the version in `Cargo.lock`
- `cargo install cargo-insta` for reviewing snapshots

Everything builds and tests on Windows, Linux and macOS. Scripts come in pairs: `.ps1` for Windows, `.sh` for the rest.

```bash
cargo test --workspace
```

## Adding a rule

1. **Pick an id**: `<area>.<name>` in lowercase kebab case, e.g. `net.socket`. Areas in use: `exec`, `code`, `net`, `mc`, `fs`, `reflect`, `deser`, `script`, `proc`, `str`, `blob`, `res`, `jar`, `meta`, `lib`, `obf`, `corr`. Ids are permanent: never rename or reuse one. To retire a rule, delete it and leave its id unused.
2. **Add it to the rule table** in `crates/core` with severity, title, why it matters and legitimate uses. Follow [Writing copy](#writing-copy).
3. **Add the matcher.** Prefer a table entry (owner, name, descriptor) over custom code. If your rule needs custom code, keep it inside core with no I/O.
4. **Add a fixture**: `fixtures/src/<rule-id>/` with the smallest Java source that triggers the rule. Follow [Fixtures](#fixtures).
5. **Build fixtures** and commit the jar:

   ```bash
   ./fixtures/build.sh
   ```

   On Windows:

   ```powershell
   .\fixtures\build.ps1
   ```

6. **Add a snapshot test** for the fixture's report, run `cargo test`, then review with `cargo insta review`. Check that the new finding has the right location and that nothing unrelated changed.
7. **Check the benign fixture** still has zero High findings. The test fails if it doesn't.
8. **Check false positives** with `scripts/fp-bench` if the rule could fire on ordinary plugins. Put the summary in your pull request.
9. **Document it** in [docs/rules.md](docs/rules.md): same title and copy as the rule table, plus exactly what triggers it.

Correlations follow the same steps. State the scope (one class or the whole jar) and make the fixture contain all parts in that scope, plus a second fixture that has only some parts and must not trigger it.

## Writing copy

Reports are read by server owners, not security engineers.

- **Titles** say what the code can do, from the owner's side: "Can give players operator", "Can run commands on your server's operating system". Data findings start with "Contains". Keep titles under 60 characters.
- **Why it matters** is one sentence about the consequence for the server or machine.
- **Legit uses** names real, common reasons a plugin does this. If you can't think of any, say "Rare" or "None known" rather than inventing some.
- **Never** use: malware, malicious (except to describe what a technique can be used for), virus, backdoor, infected, dangerous, safe, clean, trusted. jarsnitch states facts and leaves the judgement to the reader. The only exception is the fixed disclaimer at the end of every report.
- Don't accuse authors. Obfuscation and license checks are normal in paid plugins.

## Fixtures

Fixtures are tiny plugins compiled against stubs in `fixtures/stubs/` (for example `org.bukkit.entity.Player` with empty method bodies), so no Paper, BungeeCord or Velocity jar is needed.

Hostile-looking code must be inert:

- Never call it. Put it in a method that nothing invokes, not in `onEnable` or a static initializer.
- Hosts use `example.invalid`. IPs use the documentation ranges `192.0.2.0/24`, `198.51.100.0/24` or `203.0.113.0/24`.
- Webhooks use an all-zero id and a token that is obviously fake, e.g. `https://discord.com/api/webhooks/000000000000000000/` followed by 68 `x` characters. Never paste a real webhook or token, even a dead one.
- Never include real malware, real indicators, or code copied from malware.
- Never load a fixture into a real server.

The build scripts compile with `javac --release 17` and package with `jar --date=2000-01-01T00:00:02Z`, so rebuilding unchanged sources gives identical jars. Fixture jars are committed; CI doesn't need a JDK.

Malformed inputs (truncated jars, garbage class files, zip bombs, zip-slip names, duplicate entries) are built in memory by the Rust tests with small `Limits`, not committed as files. Rules about the archive's structure (`res.type-mismatch`, `jar.nested`, `jar.unsafe-path`) are tested the same way in `crates/core/tests/robustness.rs` and listed in `IN_MEMORY` in `crates/core/tests/fixtures.rs`.

Required fixtures besides one per rule:

- `benign-plugin`: an ordinary plugin with commands, a config, a listener and an update check. Must produce zero High findings.
- `diff-v1` and `diff-v2`: the same plugin; v2 adds a Discord webhook and a `URLClassLoader`. The diff test asserts both appear under "adds" and nothing else does.

## False-positive bench

`scripts/fp-bench/` checks rules against real, popular plugins. It isn't part of CI.

- `plugins.txt` lists one Modrinth project slug per line. The scripts ask the Modrinth API for each project's latest Bukkit-family release (Velocity or BungeeCord if there's none) and download it from Modrinth. `run.ps1 -List` / `run.sh --list` prints the resolved URLs without downloading.
- `run.ps1` / `run.sh` download into `scripts/fp-bench/cache/` (gitignored), run `jarsnitch scan cache/ --json`, and print one line per plugin: name, version, High / notable / info counts, and the High rule ids.
- Never add real malware or a link to it, including "for testing".

A rule that gives a High finding on a popular, reputable plugin needs either a tighter trigger or a clear explanation in its legit-uses copy.

## Known-bad indicators

v1 ships an empty indicator list. To add an entry (a file hash or a host):

- Put a comment next to it with a public source URL: a published advisory, a security write-up or an official project announcement.
- Private reports, other tools' signature databases, and "I saw it on Discord" don't count.
- Never invent hashes, hosts or signatures, and never use real indicators in fixtures.

## Report schema changes

Adding a field to the report is fine. Renaming, removing or changing the meaning of a field requires bumping `schema_version` and saying so in the pull request. See [docs/architecture.md](docs/architecture.md#report-format).

## Pull request checklist

- [ ] `cargo test --workspace` passes on your platform
- [ ] New or changed rules have a fixture and a reviewed snapshot
- [ ] Benign fixture still has zero High findings
- [ ] `docs/rules.md` matches the rule table
- [ ] Fixtures are inert and contain no real hosts, IPs, webhooks or tokens
- [ ] No new dependency in `crates/core` without a reason in the pull request

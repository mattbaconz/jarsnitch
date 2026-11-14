# Benchmarks

jarsnitch on the 100 most-downloaded plugins on Modrinth. Measured on 13 November 2026 with a pre-release build of jarsnitch 0.2.1: 0.2.0 plus the descriptor fix, the only engine change in 0.2.1 (see the [changelog](../CHANGELOG.md)), on an AMD Ryzen 7 7735HS laptop (16 GB, Windows 11). Every time is the median of 3 runs.

## Corpus

[`scripts/bench/corpus.json`](../scripts/bench/corpus.json) pins, by URL and SHA-512, each project's latest release for Paper, Spigot, Bukkit, Folia or Purpur (Velocity, BungeeCord or Waterfall if it has none) and the release before it.

- 100 latest jars: 368 MB, 145,458 classes
- 91 of them have a previous release to compare against

› scripts/bench/corpus.json · Modrinth top 100 · snapshot of 13 Nov 2026

## Speed

Native CLI, one thread, wall time including process start.

| Run | Time |
|---|---|
| `jarsnitch scan` on the folder of 100 jars | 11.74 s (31 MB/s, about 12,400 classes/s) |
| One jar per process, median | 80 ms |
| One jar per process, 90th percentile | 482 ms |
| `jarsnitch diff`, previous release against latest, median | 152 ms |

| Slowest jars | Size | Scan |
|---|---|---|
| Geyser | 52 MB | 1.49 s |
| AuthMe ReReloaded | 23 MB | 1.24 s |
| EssentialsX | 5 MB | 0.92 s |
| TAB | 8 MB | 0.91 s |
| Terra | 12 MB | 0.59 s |
| Customizable Player Models | 3 MB | 0.58 s |
| DiscordSRV | 11 MB | 0.58 s |

The website runs the same engine compiled to WebAssembly. In Node v26.10.0 (V8, the JavaScript engine in Chrome) it scans the same 100 jars in 14.14 s, Geyser in 1.69 s. The deployed 0.2.0 `jarsnitch.wasm` is 213 KB over the network (brotli). The Windows CLI is a single binary under 1 MB and needs no Java.

› jarsnitch 0.2.1 · native CLI on one thread, WebAssembly in Node v26.10.0 · median of 3 runs · Ryzen 7 7735HS · 13 Nov 2026

## What the top 100 can do

Plugins with at least one finding of each rule, out of 100. Plugins are counted, never named. Most of this is ordinary plugin behavior; [rules.md](rules.md) lists common legitimate uses for each rule. A finding counts as reached unless jarsnitch marked it unreached.

<!-- bars -->

| Rule | Severity | Plugins | Reached |
|---|---|---|---|
| `str.url` | info | 93 | 93 |
| `net.http` | info | 70 | 68 |
| `mc.console-command` | info | 27 | 26 |
| `code.load` | notable | 21 | 17 |
| `fs.write-archive` | notable | 18 | 13 |
| `deser.java` | notable | 17 | 9 |
| `net.socket` | notable | 15 | 12 |
| `proc.exit` | notable | 15 | 8 |
| `exec.process` | notable | 14 | 11 |
| `code.native` | notable | 11 | 11 |
| `jar.nested` | info | 8 | 8 |
| `mc.set-op` | notable | 8 | 7 |
| `reflect.indirect` | notable | 6 | 4 |
| `code.instrument` | notable | 5 | 2 |
| `corr.download-and-run` | high | 3 | 1 |
| `str.ip` | notable | 2 | 2 |
| `script.engine` | notable | 2 | 2 |
| `corr.command-and-network` | high | 2 | 1 |
| `res.type-mismatch` | notable | 1 | 1 |
| `corr.modify-plugins` | high | 1 | 1 |

› jarsnitch 0.2.1 · latest release of each of the 100 plugins · 13 Nov 2026

## Call graph

- The 100 jars have 142 notable or high findings. 39 of them (27%) sit in code that no entry point calls; jarsnitch marks those and lists them last.
- The call graph was built for all 100 jars. In 62, reachable code also loads code at runtime, which no static call graph can follow, so code marked unreached may still run. Their reports set `dynamic`.

› jarsnitch 0.2.1 · 100 latest releases · 13 Nov 2026

## High findings

6 plugins have a High finding, 3 of them in reached code. All six were checked by hand: demo code from a bundled library (2, unreached), a test class shipped in the jar (1, unreached), a library downloader (1), a built-in updater (1) and a database's bundled server tool (1).

› jarsnitch 0.2.1 · each High finding checked by hand · 13 Nov 2026

## Real updates

The 91 previous-to-latest diffs:

- None added a notable or high finding.
- 27 added smaller items: 54 info findings, 108 permissions, 20 bundled libraries, 20 optional dependencies, 8 commands, 4 Maven libraries, and 14 new classes with notable evidence for rules the old version already triggered.
- 5 changed their authors or website.
- None were re-obfuscated.

› jarsnitch 0.2.1 · 91 diffs, previous release against latest · 13 Nov 2026

## Limits

- Modrinth ranks a project by all its downloads, so a project that also ships mods ranks by those too.
- The times come from a laptop in everyday use, not a quiet benchmark machine.
- The corpus is one snapshot. Newer releases will give different numbers.
- The WebAssembly times come from Node, not a browser tab.
- Findings say what code can do, not what it's for.

## Reproduce

```sh
cargo build --release -p jarsnitch
python3 -I scripts/bench/bench.py fetch CACHE      # about 720 MB, each file checked against its SHA-512
python3 -I scripts/bench/bench.py run CACHE OUT    # writes OUT/results.json and OUT/summary.md
scripts/build-web.sh                               # or build-web.ps1; writes web/jarsnitch.wasm
node scripts/bench/wasm.mjs CACHE/latest OUT/wasm.json
```

`bench.py corpus` pins today's top 100 instead. Nothing here runs the jars. `results.json` names plugins next to their findings, which is why this page only counts them.

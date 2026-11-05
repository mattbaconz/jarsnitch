# Architecture

One Rust core, two front ends. The core turns bytes into a `Report`; the CLI and the web page only read files and render reports.

## Layout

```text
jarsnitch/
  Cargo.toml            workspace
  crates/
    core/               jarsnitch-core: bytes in, Report out
    cli/                jarsnitch: builds the `jarsnitch` binary
    wasm/               jarsnitch-wasm: exports scan and diff to JavaScript
  web/                  static site: index.html, style.css, app.js, worker.js, engine.js, jarsnitch.wasm (built)
  fixtures/
    stubs/              minimal API stubs (org.bukkit..., net.md_5..., com.velocitypowered...)
    src/<fixture>/      Java sources, one folder per fixture
    jars/               built fixture jars (committed)
    build.ps1, build.sh
  scripts/
    build-web.ps1, build-web.sh
    fp-bench/           plugins.txt, run scripts, cache/ (gitignored)
  docs/
```

## Core

Rules for `crates/core`:

- No I/O, network, threads, clock or randomness. `#![forbid(unsafe_code)]`.
- Never panics on input. Problems with the input become warnings in the report.
- Deterministic: the same bytes give byte-identical JSON on every platform. Paths always use `/`; collections are sorted or ordered maps.

```rust
pub fn scan(name: &str, bytes: &[u8], limits: &Limits) -> Report;
pub fn diff(old_name: &str, old: &[u8], new_name: &str, new: &[u8], limits: &Limits) -> Diff;
```

`Limits::default()` is what the CLI and the web page use. Tests pass small limits so zip-bomb and depth tests stay fast.

### Pipeline

1. Hash the file (SHA-256) and open the zip from its central directory, as the JVM's `JarFile` does.
2. Walk entries in central-directory order, enforcing [limits](#limits-and-robustness) and cleaning names.
3. Classify each entry by its bytes first and its name second: class, archive, descriptor, `pom.properties`, manifest, text resource, other.
4. Classes: parse the constant pool, methods and `Code` attributes and record facts: call sites, string literals with their location, superclass and interfaces, event-handler signatures, comparisons against string literals.
5. Archives (nested, disguised, decoded): recurse with depth + 1 into the same fact store, prefixing locations with `path!/`.
6. Strings and text resources: extract URLs, IPs and webhooks; decode base64 and hex candidates and feed results back to step 4 or 5.
7. Resolve reflection targets, then match all facts against the rule tables to produce findings.
8. Evaluate correlations over the findings.
9. Compute the obfuscation score; parse descriptors and libraries.
10. Sort everything and build the `Report`.

### Class parsing

`crates/core/src/class.rs` is a minimal parser: constant pool, this/super/interfaces, field and method names and descriptors, `Code` and `BootstrapMethods`. Every other attribute is skipped by length, so new class-file versions parse as long as the constant pool doesn't gain new tags. Errors are returned, never panics. The bytecode walker uses the opcode-length table, including `tableswitch` / `lookupswitch` padding and `wide`, and stops at the first invalid opcode.

The `cafebabe` crate was considered and not used: jarsnitch needs only a small part of the format, and its own parser keeps the wasm small and the failure behaviour under its control.

`CAFEBABE` is also the magic of a Mach-O universal binary. Treat bytes as a class file only if the major version (bytes 6–7) is at least 45; a smaller value there is the binary's architecture count.

## Report format

`crates/core` serializes `Report` with serde. Abridged example:

```json
{
  "schema_version": 1,
  "tool_version": "0.1.0",
  "file": { "name": "ExamplePlugin.jar", "size": 48213, "sha256": "9f2c..." },
  "partial": false,
  "plugins": [
    {
      "platform": "bukkit",
      "descriptor": "plugin.yml",
      "name": "ExamplePlugin",
      "version": "1.2.0",
      "main": "com.example.ExamplePlugin",
      "commands": ["example"],
      "permissions": ["example.use"],
      "folia_supported": false,
      "maven_libraries": []
    }
  ],
  "libraries": [
    { "group": "com.google.code.gson", "artifact": "gson", "version": "2.10.1",
      "location": "META-INF/maven/com.google.code.gson/gson/pom.properties" }
  ],
  "nested": [ { "path": "libs/helper.jar", "size": 10240, "sha256": "41ab..." } ],
  "obfuscation": { "score": 4, "short_names": 0.05, "default_package": 0.0, "encoded_strings": 0.01 },
  "findings": [
    {
      "rule": "str.url",
      "severity": "info",
      "title": "Contains web addresses",
      "why": "These are hosts the plugin may contact. Unfamiliar ones are worth a look.",
      "legit_uses": "Update checks, statistics, documentation links, APIs.",
      "value": "api.example.invalid",
      "evidence": [
        { "location": "com/example/UpdateChecker", "method": "check()V",
          "detail": "https://api.example.invalid/v1/latest" }
      ],
      "evidence_total": 1
    }
  ],
  "warnings": [ { "code": "class.malformed", "location": "a/B.class", "message": "truncated constant pool" } ],
  "stats": { "entries": 42, "classes": 30, "uncompressed_bytes": 120000 },
  "disclaimer": "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee."
}
```

- `value` is `null` for capability rules and holds the host, IP, webhook id, path, class name or hash for value-carrying rules.
- `evidence` is capped at 50 entries per finding; `evidence_total` gives the real count.
- Ordering: findings by severity (high first), then rule id, then value. Evidence by location, method, detail, deduplicated.
- `platform` is one of `bukkit`, `paper`, `bungee`, `velocity`, `fabric`, `quilt`, `forge`, `neoforge`.
- Stability: adding a field is not a breaking change. Renaming, removing or changing the meaning of a field bumps `schema_version`. Rule ids never change.
- No timestamps. Snapshot tests redact `tool_version`.

### Warning codes

| Code | When |
|---|---|
| `zip.unreadable` | Not a zip, or the central directory can't be read. For the scanned file itself the report has no findings; a nested archive is skipped. |
| `zip.limit` | A size, count or ratio limit was hit. Sets `partial`. |
| `zip.duplicate-entry` | Two entries share a name. Both are scanned. |
| `text.host-list` | A text file holds over 100 bare hostnames, so it's taken for a domain list (a public suffix list, say) and hostnames in it aren't reported. IPs and webhooks still are. |
| `zip.unsupported` | Encrypted entry or a compression method other than stored/deflate. Sets `partial`. |
| `zip.bad-entry` | An entry's local header or compressed data is corrupt. It's skipped. Sets `partial`. |
| `class.malformed` | A class file couldn't be parsed. It's skipped. |
| `descriptor.malformed` | A descriptor couldn't be parsed. |
| `nested.too-deep` | An archive beyond depth 3 wasn't scanned. Sets `partial`. |
| `decode.limit` | The decoded-payload budget ran out. Sets `partial`. |

The zip reader (`crates/core/src/archive.rs`) reads the central directory the way the JVM does: it finds the end record from the back, supports zip64, corrects offsets when data is prepended (shell-script launchers), and keeps duplicate names as separate entries.

## Diff

`diff` scans both jars and compares the results. It takes bytes, not reports, because it also needs every class name, which the report doesn't carry. Findings are matched by a key, not by evidence, so moved code isn't reported as a change.

| What | Key |
|---|---|
| Capability, correlation and metadata findings, `lib.log4j-vulnerable`, `obf.score` | rule id |
| `str.url`, `str.ip` | rule id + host / address |
| `str.discord-webhook`, `str.telegram-bot` | rule id + webhook / bot id |
| `blob.embedded-code` | rule id + SHA-256 of decoded bytes |
| `res.type-mismatch`, `jar.unsafe-path`, `jar.nested` | rule id + path |
| Bundled libraries (`pom.properties`) and descriptor `libraries:` | `group:artifact`; a version change is listed under changes |
| Nested jars | path; added and removed ones are `jar.nested` findings, a different SHA-256 is listed under changes |
| Commands, permissions | the value |
| Classes | location (`a/B`, `lib.jar!/a/B`) |

Sections, in this order:

1. **Adds:** new findings, new commands and permissions, new libraries, and new classes that carry notable or high evidence. Sorted by severity, then findings before classes before the rest.
2. **Removes:** the same kinds, gone from the new version.
3. **Changes:** library versions, nested jar hashes, plugin name, version or main class.
4. **Unchanged:** findings present in both, by rule id (and value).

The header shows both file names, versions, short hashes and the obfuscation score old -> new. If either file isn't a readable zip, the CLI prints an error and exits 1 instead of a diff.

If fewer than half of the old class names exist in the new jar and either side scores 25+ for obfuscation, the diff adds the warning `diff.classes-renamed`: the jar was probably re-obfuscated, so new and removed classes are left out for this pair. Findings are still compared. A partial scan on either side adds `diff.partial`.

A capability that already existed but now appears in many more places counts as unchanged in v1.

```json
{
  "schema_version": 1,
  "old": { "file": { "name": "Example-1.0.jar", "size": 1304, "sha256": "..." }, "plugins": [ ... ], "partial": false, "warnings": [] },
  "new": { "file": { "name": "Example-1.1.jar", "size": 1610, "sha256": "..." }, "plugins": [ ... ], "partial": false, "warnings": [] },
  "obfuscation": { "old": 4, "new": 5 },
  "added":   [ { "kind": "finding", "finding": { "rule": "code.load", "...": "..." } },
               { "kind": "class", "value": "com/example/Loader", "rules": ["code.load"] } ],
  "removed": [ { "kind": "finding", "finding": { "rule": "str.url", "value": "old-api.example.invalid", "...": "..." } } ],
  "changed": [ { "kind": "library", "key": "com.google.code.gson:gson", "old": "2.10.1", "new": "2.11.0" },
               { "kind": "plugin-field", "key": "plugin.yml version", "old": "1.0", "new": "1.1" } ],
  "unchanged": [ { "rule": "net.http", "value": null } ],
  "warnings": [],
  "disclaimer": "..."
}
```

In `added` and `removed`, `kind` is one of `finding`, `class`, `command`, `permission`, `maven-library` (a descriptor `libraries:` entry) and `library` (bundled, from `pom.properties`); every kind except `finding` and `class` carries its `value`. In `changed`, `kind` is `library`, `maven-library`, `nested` or `plugin-field` (`name`, `version` or `main` of one descriptor), and `old` / `new` are `null` when a side doesn't have it.

## CLI

- Argument parsing with `std::env::args`; two subcommands and one flag don't need a framework.
- Default output is plain text: a header per jar (name, version, platform, short hash, obfuscation score), then one block per finding with severity, title, rule id, the "why" line and up to 3 locations plus "+N more", then severity counts and the disclaimer. Partial reports say so at the top.
- `--json` prints the serde output: an array of reports for `scan`, one object for `diff`.
- Exit codes are listed in the [README](../README.md#command-line-reference).
- Folder scans read every `.jar` directly in the folder, sorted by name, one at a time.

## Web app

```text
web/
  index.html    two tabs: Scan (one drop zone), Compare (old + new drop zones)
  style.css
  app.js        UI and rendering
  worker.js     module worker: fetches and compiles the wasm, runs scan and diff
  engine.js     the calls into the wasm, shared with scripts/check-web.mjs
  jarsnitch.wasm  built by scripts/build-web.ps1 / .sh, not committed
  _headers      the CSP and other headers for hosts that read this file
```

`crates/wasm` exports its memory and five plain functions instead of using wasm-bindgen: `input(slot, len)` returns where JavaScript writes an input (0 old name, 1 old bytes, 2 new name, 3 new bytes; `scan` uses 0 and 1), `scan()` and `diff()` return the length of the JSON result, `output_ptr()` says where it is, and `clear()` frees everything. There is no generated glue and no `unsafe` block, and building needs only `cargo` and the wasm32 target.

Flow:

1. On page load, `app.js` starts the worker right away. The worker fetches and compiles `jarsnitch.wasm` and posts `ready`. Everything is fetched now, so nothing is fetched later.
2. The user drops or picks files (each drop zone is also a keyboard-reachable `<input type="file">`). Dropping one jar anywhere scans it; dropping two compares them, older name first (`Foo-1.9.jar` before `Foo-1.10.jar`), with a Swap button.
3. `app.js` reads `file.arrayBuffer()` and posts it to the worker as a transferable (no copy).
4. The worker runs `scan` or `diff` through `engine.js` and posts the parsed result back. If the wasm traps, the worker starts a fresh instance for the next request.
5. `app.js` renders the report in the same order as the CLI and offers "Download JSON", built locally from a `Blob`. Findings that share a rule (hosts, say) share one card; info findings start collapsed. Report text comes from the jar, so it's only ever inserted as text, never as HTML.

Privacy rules:

- No third-party requests of any kind: no fonts, analytics, CDNs or remote images.
- `index.html` carries a Content Security Policy:

  ```text
  default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self'; style-src 'self'; img-src 'self' data: blob:; base-uri 'none'; form-action 'none'
  ```

  A `<meta>` CSP doesn't cover the worker. Hosts that support response headers (for example a `_headers` file) should send the same policy as a header.
- Check: open DevTools > Network, load the page, clear the log, run a scan and a diff. The log must stay empty. Then go offline and repeat; both must still work.

Accessibility and layout basics: a status line in an `aria-live` region announces each result in one sentence and focus moves to the result heading (reading a whole report aloud on every scan would be too much), severity is shown as text and not only as color, light and dark themes follow `prefers-color-scheme`, tabs follow the ARIA tabs pattern with arrow keys, and the page works at phone width without sideways scrolling.

Speed: on a laptop, a 52 MB plugin (Geyser) scans in about 1.5 s in Chromium, 2 to 3 s in WebKit and 10 to 11 s in Firefox, measured through Playwright's browser builds. The worker keeps the tab responsive either way.

## Limits and robustness

Defaults, defined as constants in `crates/core`:

| Limit | Default |
|---|---|
| Total uncompressed bytes, whole tree | 512 MiB |
| Single entry, uncompressed | 128 MiB |
| Entries, whole tree | 100,000 |
| Compression ratio for entries over 1 MiB | 200 : 1 |
| Nesting depth (top-level jar is 0) | 3 |
| Text resource scanned for strings | 1 MiB |
| Descriptor file | 1 MiB |
| Decoded payloads per scan | 1,000, 16 MiB total |

When a limit is hit, that part stops, a warning is added and everything found so far is kept. Entries are decompressed one at a time, so memory stays near the size of the largest entry plus the input.

YAML descriptors are read by a small hand-written parser that knows only the subset descriptors use (top-level scalars, the keys of `commands` and `permissions`, the `libraries` list). It has no anchors or aliases, so a small file can't expand into a huge document.

## Build and test

```bash
cargo test --workspace
```

```bash
cargo run -p jarsnitch -- scan fixtures/jars/benign-plugin.jar
```

```bash
rustup target add wasm32-unknown-unknown
```

```bash
./scripts/build-web.sh
```

On Windows, `scripts\build-web.ps1`. Both run `cargo build -p jarsnitch-wasm --release --target wasm32-unknown-unknown` and copy the result to `web/jarsnitch.wasm` (about 0.5 MB). Release builds use `lto = true`, `codegen-units = 1`.

```bash
node scripts/check-web.mjs
```

Loads the built wasm through `web/engine.js`, the code the page runs, and checks a scan, a diff and a broken file. Pass jar paths to also scan and time them.

Any static server works for local testing, for example:

```bash
python -m http.server 8080 -d web
```

Fixture jars need JDK 17+ (`javac --release 17`, `jar --date=...` for reproducible output). See [CONTRIBUTING.md](../CONTRIBUTING.md).

Windows: commit a `.gitattributes` with `*.jar binary`, `*.snap text eol=lf` and `*.sh text eol=lf`, so checkouts don't change fixture bytes or snapshot line endings.

## Dependencies

Keep the list short; every crate in `core` also ships in the wasm.

| Crate | Used by | For |
|---|---|---|
| `miniz_oxide` | core | Inflating zip entries, with an output cap. Pure Rust, builds for wasm32. |
| `serde`, `serde_json` | core | Report types, JSON, `velocity-plugin.json` |
| `sha2` | core | File and payload hashes |
| `insta` (json, redactions) | dev | Snapshot tests |
| `miniz_oxide` | dev | Deflating entries of archives built in tests |

The zip reader, class parser, YAML subset and base64/hex decoders are written in `core`. Each is small, and owning them keeps the limits and the never-panic rule in one place.

Avoid `regex` in core (wasm size; the patterns here are simple enough to scan by hand), `clap` in the CLI, and terminal color crates.

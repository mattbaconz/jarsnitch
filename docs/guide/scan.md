# Scan a plugin

Scan one jar, several jars or a whole plugins folder, on the website or with `jarsnitch scan`. Each jar gets its own report.

## On the website

<!-- gif: site-scan | The jarsnitch page: SampleShop-1.4.1.jar is dropped on the Scan tab, the report shows 2 high, 4 notable and 5 info findings and a map of where they are in the jar, and clicking ChatListener on the map jumps to its finding's call path from onEnable to the matched code. -->

| You drop or choose | What happens |
|---|---|
| One file | It's scanned and its report opens. The picker takes `.jar` and `.zip`. |
| Two jars with the same name stem, like `Foo-1.9.jar` and `foo_2.0.jar` | They're compared as two versions of one plugin, older name first. See [Compare two versions](compare.md). |
| Any other set of jars | Each is scanned into one list. |
| A folder, such as your `plugins` folder | Every `.jar` directly inside it is scanned into one list. Subfolders are skipped, and a folder is never compared, even when it holds two jars. |

The status line under the drop zone says what's happening: `Analyzing SampleShop-1.4.1.jar…`, then the result with the time it took, like `Scanned SampleShop-1.4.1.jar in <time>: 2 high, 4 notable, 5 info.` For several jars it counts through them (`Scanning 2 of 3: …`) and ends with how many jars have high or notable findings and how many were unreadable. A folder with no jar directly in it says so.

### The list of jars

Several jars give one row per jar with its severity counts. Rows are ranked by most High findings, then most Notable, then by name, so the jars worth a look come first. A file that isn't a readable jar or zip stays in the list, marked as such. Open a row to see that jar's full report.

### Download JSON

**Download JSON** saves what's on screen, built in the page: one report object for one jar, an array of reports for several. The fields are described in [JSON format](json.md).

### Try a sample plugin

**Try a sample plugin** scans SampleShop 1.4.1, a made-up plugin that ships with the site. Its report has 2 High findings, so it shows every part of a report. Its code never runs; it's read like any other jar.

## On the command line

```bash
jarsnitch scan MyPlugin.jar
```

```bash
jarsnitch scan plugins/
```

```bash
jarsnitch scan plugins/ other/Extra.jar --json > reports.json
```

<!-- gif: cli-scan | jarsnitch scan SampleShop-1.4.1.jar run in a terminal: two High findings print first, then the notable and info ones and the count: 2 high, 4 notable, 5 info. -->

- `scan` takes one or more files or folders, in any mix.
- **Folders** are read one level deep: every file directly inside whose name ends in `.jar`, in any case. Subfolders are skipped. Files are scanned in byte order of their path, so `Foo-1.10.jar` comes before `Foo-1.9.jar`. (The website sorts version numbers numerically instead.)
- **A file you name** is read as a zip whatever its extension. If it isn't one, its report has a `zip.unreadable` warning and no findings.
- **Reports** go to standard output: one block per jar, then the disclaimer once at the end. With `--json`, a JSON array with one report per jar.
- **Problems** go to standard error and the scan carries on with the other files:

```out $ jarsnitch scan empty/
warning: no .jar files in empty/
```

```out $ jarsnitch scan notes.txt
== notes.txt ==
sha256 ee3a20bc47f5 · 0 classes · obfuscation 0/100

Warnings:
  zip.unreadable         no end-of-central-directory record

0 high · 0 notable · 0 info

This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.
```

› real output: jarsnitch 0.2.1 · both exit with code 1

A file that doesn't exist prints `error: <path>: <reason from the operating system>` and also makes the exit code 1.

## Exit codes

| Code | When |
|---|---|
| 2 | At least one jar has a High finding. This wins even if another file couldn't be read. |
| 1 | No High finding, but a file was missing or unreadable, or nothing was scanned at all (for example a lone folder with no jars). A folder with no jars next to other files only prints a warning. |
| 0 | Every jar was read and none has a High finding. |

Use 2 to stop an update script or fail a CI job. Code 1 means jarsnitch couldn't check everything you asked for, so treat it as a stop too. More in [Command line](cli.md).

## Speed

A typical plugin takes well under a second. Across Modrinth's 100 most-downloaded plugins, one jar per process took 80 ms at the median and 482 ms at the 90th percentile. The largest, Geyser at 52 MB, took 1.49 s. The website runs the same engine compiled to WebAssembly: in V8, the JavaScript engine in Chrome, Geyser took 1.69 s. The page stays responsive while it works.

› jarsnitch 0.2.1 · native CLI, one thread, and WebAssembly in Node 26 (V8) · median of 3 runs · Ryzen 7 7735HS laptop · 13 Nov 2026 · [Benchmarks](../benchmarks.md#speed)

## Large or damaged jars

Scans stop at fixed size and count limits, so a zip bomb can't exhaust memory. When a limit is hit, the report keeps everything found so far and is marked partial. A partial report is not a clean one: part of the jar wasn't checked. See [Limits](limits.md).

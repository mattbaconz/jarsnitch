# Command line

`jarsnitch` is a single binary with three commands: `scan`, `diff` and `check`. It reads the files you name and writes a report. It needs no Java, reads no config file or environment variable and prints no colour. Only `check` uses the network; see [Check against official releases](check.md).

## Synopsis

<!-- include: README.md#command-line-reference -->

## Help and version

```out $ jarsnitch --help
jarsnitch: see what a Minecraft plugin can do before you install it

usage:
  jarsnitch scan <jar|folder>... [--json]
  jarsnitch diff <old.jar> <new.jar> [--json]
  jarsnitch check <jar|folder>... [--mc <version>] [--no-updates] [--json]
  jarsnitch --version

A folder means every .jar directly inside it.
check compares each jar with the official release on Modrinth and Hangar and looks
for updates there. It's the only command that uses the network.
Exit codes: 0 no High findings (diff: none added; check: no jar's code differs from
its official file and nothing adds a High finding), 1 a file couldn't be read, a
request failed or there was nothing to scan, 2 at least one High finding (diff: the
update adds one; check: a jar's code differs from its official file, or the jar or
its update adds a High finding).
```

› real output: jarsnitch 0.4.0

- `-h` or `--help` anywhere on the line, or `help` as the first word, prints this and exits with 0.
- `-V` or `--version` prints `jarsnitch 0.4.0` and exits with 0.

## Options and arguments

- **`--json`** can go anywhere on the line: `jarsnitch scan --json plugins/` works too.
- **`--mc <version>`** (or `--mc=<version>`) and **`--no-updates`** only go with `check`. With another command they print an error and the usage, and exit with 1.
- **Any other word starting with `-`** prints `error: unknown option -x`, then the usage, and exits with 1.
- **The wrong number of arguments** (`diff` with one file, `scan` with none, an unknown command) prints the usage and exits with 1.
- **Names in reports** are the file name only, never the folder it's in.

## Where output goes

- Reports and JSON go to standard output.
- Lines starting with `error:` or `warning:` go to standard error, so `--json > report.json` stays valid JSON.
- `check` also prints one line to standard error before it sends anything, saying what it sends and to which sites.

## Severity and exit codes

| Severity | Meaning | `scan` exits | `diff` exits | `check` exits |
|---|---|---|---|---|
| info | Worth knowing. Common in ordinary plugins. | 0 | 0 | 0 |
| notable | A powerful capability. Check that it fits what the plugin says it does. | 0 | 0 | 0 |
| high | A combination of capabilities, or a library with known security bugs, that gives the jar a lot of control over your server or machine. Still not proof of bad intent. | 2 | 2 if the update adds it | 2 if the update adds it |

When several things apply, the first match decides:

| Command | Exit code |
|---|---|
| any | 0 for `--help` and `--version`; 1 for an unknown option or the wrong arguments. |
| `scan` | 2 if any jar has a High finding, even if another file failed. Otherwise 1 if a file was missing or unreadable, or nothing was scanned at all (for example a lone folder with no jars); a folder with no jars next to other files only prints a warning. Otherwise 0. |
| `diff` | 1 if either file is missing or isn't a readable zip; nothing is printed to standard output. Otherwise 2 if the update adds a High finding. Otherwise 0. |
| `check` | 2 if a jar's code differs from the official file of its version, or a jar or its update adds a High finding, even if another file or request failed. Otherwise 1 if a file was missing or unreadable, a request failed, or there was nothing to check. Otherwise 0. A jar that isn't on Modrinth or Hangar doesn't change it. |

Reachability never changes an exit code: a High finding in code the plugin doesn't call still exits with 2.

## Text output

`scan` prints one block per jar, then the disclaimer once. Every line is explained in [Read a report](report.md#the-command-line-report-line-by-line).

<!-- gif: cli-scan | jarsnitch scan SampleShop-1.4.1.jar run in a terminal: two High findings print first, then the notable and info ones and the count: 2 high, 4 notable, 5 info. -->

`diff` prints:

| Line | Meaning |
|---|---|
| `== old.jar -> new.jar ==` | The two file names. |
| `SampleShop 1.4.0 -> 1.4.1` | Name and both versions (both names when they differ). |
| `sha256 … -> … · obfuscation 0 -> 0` | Short hashes and obfuscation scores. |
| `WARNING: …` | A [note](compare.md#notes-a-comparison-can-add): re-obfuscated jar, a native library that differs, partial scan, or the same version with different code. |
| `This update ADDS:` | Then each added item; `This update ADDS: nothing` when there are none. |
| `HIGH     New class: a/B   [rules]` | A new class with notable or High evidence, and the rules it triggered. |
| `INFO     New command: /shop` | Also `New permission:`, `New dependency:`, `New optional dependency:`, `New downloaded library:` and `New bundled library:`. |
| `This update REMOVES:` | The same kinds, starting with `Removed` instead of `New`. |
| `Changes:` | Lines like `plugin.yml version: 1.4.0 -> 1.4.1`, `bundled library group:artifact: 1.0 -> 1.1`, `downloaded library …` and `nested jar libs/x.jar sha256: … -> …`. `(none)` stands for a missing value. |
| `A different author or website can mean the plugin has a new owner.` | After Changes, when the authors or website changed. |
| `Code: 1 class changed, 2 added, 0 removed, 3 unchanged` | The [class comparison](compare.md#code); `Code: the same in both (N classes).` when both jars run the same code (`same_code` in the JSON). Left out for a re-obfuscated jar. |
| `INFO     Changed class: a/B   [rules]` | A class whose code changed, with the notable and High rules its new code adds. Then `~ more than one entry has this name` when a jar has different copies of the class, `~ superclass, interfaces, access flags or annotations` when the declaration changed, up to 8 lines of `+ method`, `- method`, `~ method`, `+ field`, `- field` and `~ field`, and `now calls:` with the methods it calls that it didn't before. Every class with notable or High rules is listed, then others up to 10 in all, then `and N more changed classes`. |
| `New classes (2): …` | Every class only the new version has, with or without findings; `Removed classes` for the old one. |
| `Other files: ~ plugin.yml` | Files that aren't classes, by content: `+` added, `-` removed, `~` changed. Jar signatures and Maven build files are left out. |
| `Unchanged: net.http, str.url (2)` | Rules found in both versions, with a count for several values. |
| The last line | The disclaimer. |

Finding blocks look the same as in `scan`. In `diff` they stay in severity order; a finding in code the plugin doesn't call is still marked "not called".

`check` prints one block per jar, then a summary. The output is in [Check against official releases](check.md#on-the-command-line).

| Line | Meaning |
|---|---|
| `== file.jar ==` | The file name. |
| `CoreProtect 24.0` | The plugin's name and version. |
| `Official file: the same bytes as …` | These bytes are a file the project published, with the project's page. |
| `That project isn't named like the plugin (…)` | After the result line of an official, same-code or differing jar, when the project's name and address don't contain the plugin's name, or the other way round. |
| `Same code as … but not the same file.` | Then `Every file is the same; only the zip around them differs.`, or `Other files differ:` and the files, with `The server or Java reads …` when one of them is a plugin descriptor, `META-INF/MANIFEST.MF` or a file in `META-INF/services/`. Then `This jar adds:` with one line for each notable or High item the jar has that the official file doesn't. |
| `CODE DIFFERS from …` | Then `official file:` with its download address, a `WARNING:` line for each note of the comparison (such as a native library that differs), `Changes:` (such as another main class or startup library), `This jar ADDS:` with what this jar has that the official file doesn't, and the Code part, as in `diff`. |
| `Not a readable jar.` | The file isn't a zip, or its central directory can't be read. |
| `Not found on Modrinth or Hangar: …` | No file with these bytes, and no official file of this version to compare with. Or the project named like the plugin has a file of this version that isn't the same plugin: then that file, and its address to compare it with `jarsnitch diff`. |
| `Update: 24.1, 2026-09-24 (…)` | The latest release, its date and its page. Then `adds no notable or High findings`, or `adds N High and N notable findings:`, with one line for each notable or High item, and `code:` with the class counts. `adds no notable or High findings, but new classes with them:` when only new classes carry findings the jar already has. `not compared:` when the release has no file for your server software. |
| `couldn't check: …` | A request failed, so the result may be incomplete. |
| `3 jars · 1 official · …` | How many jars got each result, and how many have an update. `(N add a High finding)` after `same code` or `available` counts the jars or updates that add one. |
| `Official means …` | After the summary, when a jar is official or has the same code. |

## Recipes

### Fail a CI job when a plugin has a High finding

GitHub Actions, with the plugins committed under `plugins/`:

```yaml
- name: Install jarsnitch
  run: |
    curl --proto '=https' --tlsv1.2 -LsSf https://github.com/mattbaconz/jarsnitch/releases/latest/download/jarsnitch-installer.sh | sh
    echo "${CARGO_HOME:-$HOME/.cargo}/bin" >> "$GITHUB_PATH"
- name: Check plugins
  run: jarsnitch scan plugins/
```

The step fails on exit code 2 (a High finding) and on 1 (a jar couldn't be read, or the folder had none), so a wrong path can't pass by accident.

### Check a plugins folder against official releases

```bash
jarsnitch check plugins/ --mc 1.21.4
```

Exit code 2 means a jar's code differs from the official file of its version, or a jar or its update adds a High finding. 1 also covers a request that failed, so a job that runs this fails when Modrinth or Hangar can't be reached.

### Check an update before you install it

`jarsnitch diff` the installed jar against the download and stop on 1 or 2. Bash and PowerShell versions are in [Compare two versions](compare.md#use-it-before-every-update).

### Keep a JSON report next to each jar

```bash
for jar in plugins/*.jar; do
  jarsnitch scan "$jar" --json > "$jar.jarsnitch.json"
done
```

Each file holds an array with one report, the same fields the website's **Download JSON** saves. Compare them over time, or search them for a rule id. The format is in [JSON format](json.md).

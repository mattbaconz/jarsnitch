# Command line

`jarsnitch` is a single binary with two commands, `scan` and `diff`. It reads the files you name and writes a report. It needs no Java, reads no config file or environment variable, prints no colour and never uses the network.

## Synopsis

<!-- include: README.md#command-line-reference -->

## Help and version

```out $ jarsnitch --help
jarsnitch: see what a Minecraft plugin can do before you install it

usage:
  jarsnitch scan <jar|folder>... [--json]
  jarsnitch diff <old.jar> <new.jar> [--json]
  jarsnitch --version

A folder scans every .jar directly inside it.
Exit codes: 0 no High findings (diff: none added), 1 a file couldn't be read or
there was nothing to scan, 2 at least one High finding (diff: the update adds one).
```

› real output: jarsnitch 0.3.0

- `-h` or `--help` anywhere on the line, or `help` as the first word, prints this and exits with 0.
- `-V` or `--version` prints `jarsnitch 0.3.0` and exits with 0.

## Options and arguments

- **`--json`** can go anywhere on the line: `jarsnitch scan --json plugins/` works too.
- **Any other word starting with `-`** prints `error: unknown option -x`, then the usage, and exits with 1.
- **The wrong number of arguments** (`diff` with one file, `scan` with none, an unknown command) prints the usage and exits with 1.
- **Names in reports** are the file name only, never the folder it's in.

## Where output goes

- Reports and JSON go to standard output.
- Lines starting with `error:` or `warning:` go to standard error, so `--json > report.json` stays valid JSON.

## Severity and exit codes

| Severity | Meaning | `scan` exits | `diff` exits |
|---|---|---|---|
| info | Worth knowing. Common in ordinary plugins. | 0 | 0 |
| notable | A powerful capability. Check that it fits what the plugin says it does. | 0 | 0 |
| high | A combination of capabilities, or a library with known security bugs, that gives the jar a lot of control over your server or machine. Still not proof of bad intent. | 2 | 2 if the update adds it |

When several things apply, the first match decides:

| Command | Exit code |
|---|---|
| any | 0 for `--help` and `--version`; 1 for an unknown option or the wrong arguments. |
| `scan` | 2 if any jar has a High finding, even if another file failed. Otherwise 1 if a file was missing or unreadable, or nothing was scanned at all (for example a lone folder with no jars); a folder with no jars next to other files only prints a warning. Otherwise 0. |
| `diff` | 1 if either file is missing or isn't a readable zip; nothing is printed to standard output. Otherwise 2 if the update adds a High finding. Otherwise 0. |

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
| `WARNING: …` | A [note](compare.md#notes-a-comparison-can-add): re-obfuscated jar or partial scan. |
| `This update ADDS:` | Then each added item; `This update ADDS: nothing` when there are none. |
| `HIGH     New class: a/B   [rules]` | A new class with notable or High evidence, and the rules it triggered. |
| `INFO     New command: /shop` | Also `New permission:`, `New dependency:`, `New optional dependency:`, `New downloaded library:` and `New bundled library:`. |
| `This update REMOVES:` | The same kinds, starting with `Removed` instead of `New`. |
| `Changes:` | Lines like `plugin.yml version: 1.4.0 -> 1.4.1`, `bundled library group:artifact: 1.0 -> 1.1`, `downloaded library …` and `nested jar libs/x.jar sha256: … -> …`. `(none)` stands for a missing value. |
| `A different author or website can mean the plugin has a new owner.` | After Changes, when the authors or website changed. |
| `Unchanged: net.http, str.url (2)` | Rules found in both versions, with a count for several values. |
| The last line | The disclaimer. |

Finding blocks look the same as in `scan`. In `diff` they stay in severity order; a finding in code the plugin doesn't call is still marked "not called".

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

### Check an update before you install it

`jarsnitch diff` the installed jar against the download and stop on 1 or 2. Bash and PowerShell versions are in [Compare two versions](compare.md#use-it-before-every-update).

### Keep a JSON report next to each jar

```bash
for jar in plugins/*.jar; do
  jarsnitch scan "$jar" --json > "$jar.jarsnitch.json"
done
```

Each file holds an array with one report, the same fields the website's **Download JSON** saves. Compare them over time, or search them for a rule id. The format is in [JSON format](json.md).

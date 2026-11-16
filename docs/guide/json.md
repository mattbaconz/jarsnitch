# JSON format

Every report is also JSON: `--json` on the command line, **Download JSON** on the website. It carries everything the text shows plus the full evidence and call paths, and its field names and rule ids are stable.

## Shapes

| Where | What you get |
|---|---|
| `jarsnitch scan … --json` | An array of reports, one per jar, even for a single jar. |
| `jarsnitch diff … --json` | One comparison object. Nothing at all when a file can't be read (exit code 1). |
| `jarsnitch check … --json` | An array with one [check](#a-check) per jar. |
| Website, one jar | One report object, saved as `<file>.jarsnitch.json`. |
| Website, several jars | An array of reports, saved as `plugins.jarsnitch.json`. |
| Website, a comparison | One comparison object, saved as `<old>-vs-<new>.jarsnitch.json`. |

## A report

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | number | `1`. Goes up only when a field is renamed, removed or changes meaning. |
| `tool_version` | string | The jarsnitch version that made the report, like `0.3.0`. |
| `file` | object | `name` (file name only), `size` in bytes, `sha256` of the whole file. |
| `partial` | boolean | A scan limit was hit, so part of the jar wasn't checked. |
| `plugins` | array | One entry per descriptor found; see below. Empty for a jar with none. |
| `libraries` | array | Bundled libraries from `pom.properties` files: `group`, `artifact`, `version`, `location`. |
| `nested` | array | Nested archives: `path`, `size`, `sha256`. |
| `obfuscation` | object | `score` from 0 to 100, plus the undamped shares `short_names`, `default_package` and `encoded_strings`, each from 0 to 1. |
| `reachability` | object or null | `entry_points` (the first 100 classes the call graph starts from), `methods`, `reached_methods` and `dynamic`. `null` when the jar has no plugin main class to start from. |
| `findings` | array | See below. |
| `warnings` | array | Problems reading the jar: `code`, `location` and `message`. The codes are listed in [Read a report](report.md#warning-codes). |
| `stats` | object | `entries`, `classes` and `uncompressed_bytes`. |
| `disclaimer` | string | "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee." |

### A plugin descriptor

| Field | Type | Meaning |
|---|---|---|
| `platform` | string | `bukkit`, `paper`, `bungee`, `velocity`, `fabric`, `quilt`, `forge` or `neoforge`. |
| `descriptor` | string | The file it came from, like `plugin.yml` or `velocity-plugin.json`. |
| `name`, `version` | string or null | As declared. |
| `main`, `bootstrapper`, `loader` | string or null | The main class, and Paper's bootstrapper and loader classes. |
| `authors` | array of strings | Declared authors. |
| `website` | string or null | Declared website. |
| `commands`, `permissions` | array of strings | Declared commands and permission nodes. |
| `dependencies`, `optional_dependencies` | array of strings | Plugins it needs, and plugins it uses when present. |
| `folia_supported` | boolean | Declares Folia support. |
| `maven_libraries` | array of strings | The `libraries:` the server downloads for the plugin at startup. |

Mods get `platform`, `descriptor`, `name` and `version`; the other fields stay empty.

### A finding

| Field | Type | Meaning |
|---|---|---|
| `rule` | string | The rule id, like `net.socket`. Rule ids never change. |
| `severity` | string | `info`, `notable` or `high`. |
| `title`, `why`, `legit_uses` | string | The finding's copy, the same as on the [rules page](../rules.md). |
| `value` | string or null | The host, IP, webhook or bot id, path, class name or hash for rules that carry one; `null` for the others. |
| `evidence` | array | Up to 50 places, sorted, always including the place `path` leads to or, when `path` is empty, a reached place: `location` (class or entry path), `method` (name and descriptor, or `null`), `detail` (the matched call or text) and `reached` (the same as the finding's `reached`, for that one place). |
| `evidence_total` | number | How many places there are in all. |
| `reached` | boolean or null | `true` when an entry point reaches at least one place, `false` when every place is code none reaches, `null` otherwise: no place is reached and some aren't code the call graph covers, or the jar has no entry point. |
| `path` | array | When `reached` is `true`: one call path from an entry point to the first reached place that has one, entry point first. Empty otherwise, and when the only reached place is a class whose static fields are used but none of whose methods run. Each item is a step; see below. |

### A step

| Field | Type | Meaning |
|---|---|---|
| `location` | string | The class, like `example/sampleshop/AddonLoader`. |
| `method` | string | Method name and descriptor, like `load(Ljava/io/File;)V`. `<init>` is a constructor and `<clinit>` a static initializer. |
| `via` | string | How the step before leads here. |

| `via` | Meaning |
|---|---|
| `entry` | The server starts the plugin here: its main class, or a class it declares. The first step is always `entry`, and no other step is. |
| `call` | A direct call, or a call through an interface or superclass. |
| `callback` | The server, a library or the JVM calls it on an object the step before creates: an event handler, a command executor, `run()`, `toString()`. |
| `static-init` | Using a class runs its static initializer. |
| `reflection` | The step before names the class for reflection, or lists its methods. |

A path is one way to get to the code, not every way. How it's chosen is in [The call graph](call-graph.md#call-paths).

## A comparison

| Field | Type | Meaning |
|---|---|---|
| `schema_version`, `tool_version`, `disclaimer` | | As in a report. |
| `old`, `new` | object | Each side's `file`, `plugins`, `partial` and `warnings`. |
| `obfuscation` | object | `old` and `new` scores. |
| `added`, `removed` | array | Items with a `kind`: `finding` (with the whole `finding`, `path` included), `class` (`value` is the class, plus `rules` and `severity`), or `command`, `permission`, `dependency`, `optional-dependency`, `maven-library` and `library`, each with a `value`. |
| `changed` | array | `kind` (`library`, `maven-library`, `nested` or `plugin-field`), `key`, and `old` and `new` values, `null` when a side doesn't have one. |
| `code` | object | The class-by-class comparison; see below. |
| `same_code` | boolean | Both jars run the same code: no class or native library differs, nor the main class or a library the server downloads at startup, there's no `diff.classes-renamed`, and both jars were read in full. **Same code** in a check means this is `true`. |
| `unchanged` | array | `rule` and `value` of findings both versions have. |
| `warnings` | array | `diff.classes-renamed`, `diff.native-changed`, `diff.partial` or `diff.same-version`, as described in [Compare two versions](compare.md#notes-a-comparison-can-add). |

### Code

| Field | Type | Meaning |
|---|---|---|
| `changed_classes` | array | Classes in both versions whose code differs, most severe first, then by name; see below. |
| `added_classes`, `removed_classes` | array of strings | Every class only one version has, with or without findings. |
| `unchanged_classes` | number | Classes whose code is the same in both. |
| `added_files`, `removed_files`, `changed_files` | array of strings | Other files, compared by content. A nested jar counts by the files inside it. Jar signature files (`META-INF/*.SF`, `.RSA`, `.DSA`, `.EC`) and the `pom.properties` and `pom.xml` under `META-INF/maven/` are left out, because a build or a new signature changes them. |

A class is named by its file's path without `.class`, like `example/sampleshop/SampleShop`, or `libs/x.jar!/a/B` inside a nested jar. Everything is empty when `diff.classes-renamed` is set.

| Field of a changed class | Type | Meaning |
|---|---|---|
| `class` | string | The class. |
| `severity` | string | That of the most severe rule in `rules_added`, else `info`; at least `notable` when `duplicate` is set. |
| `rules_added` | array of strings | Notable and High rules with evidence in this class that the old class didn't have. |
| `header_changed` | boolean | The superclass, interfaces, class access flags or class annotations differ. Always `false` when `duplicate` is set. |
| `methods_added`, `methods_removed`, `methods_changed` | array of strings | Methods as `name(descriptor)`. A changed method has the same name and descriptor and different code, access flags, try/catch handlers or annotations. |
| `fields_added`, `fields_removed`, `fields_changed` | array of strings | Fields as `name:descriptor`. A changed field has different access flags, a different constant value or different annotations. |
| `calls_added` | array of strings | Methods the class calls now that it didn't before, as `owner.name`. |
| `duplicate` | boolean | More than one entry in a jar has this class's name, with different code. Their fingerprints are combined in any order, so the class counts as changed unless the other jar has the same copies. |

The sample update's `code`:

```json
{
  "changed_classes": [
    {
      "class": "example/sampleshop/SampleShop",
      "severity": "info",
      "rules_added": [],
      "header_changed": false,
      "methods_added": [],
      "methods_removed": [],
      "methods_changed": [
        "onEnable()V"
      ],
      "fields_added": [],
      "fields_removed": [],
      "fields_changed": [],
      "calls_added": [
        "example/sampleshop/AddonLoader.<init>",
        "example/sampleshop/AddonLoader.load",
        "example/sampleshop/ChatListener.<init>",
        "example/sampleshop/SampleShop.getDataFolder",
        "example/sampleshop/SampleShop.getServer",
        "org/bukkit/Server.getPluginManager",
        "org/bukkit/plugin/PluginManager.registerEvents"
      ],
      "duplicate": false
    }
  ],
  "added_classes": [
    "example/sampleshop/AddonLoader",
    "example/sampleshop/ChatListener"
  ],
  "removed_classes": [],
  "unchanged_classes": 3,
  "added_files": [],
  "removed_files": [],
  "changed_files": [
    "plugin.yml"
  ]
}
```

› real output: jarsnitch 0.4.0, `diff SampleShop-1.4.0.jar SampleShop-1.4.1.jar --json`, the `code` field

## A check

| Field | Type | Meaning |
|---|---|---|
| `file` | string | The file name. |
| `sha256` | string | SHA-256 of the whole file. |
| `plugin`, `version` | string or null | The plugin's name and version from its descriptor. |
| `status` | string | `official`, `same-code`, `differs`, `not-found` or `unreadable`; see [What you get for each jar](check.md#what-you-get-for-each-jar). |
| `official` | object or null | The official file the jar matched or was compared with. `null` when nothing was found. |
| `unlike` | object or null | With `not-found`: a file of this name and version in the project named like the plugin that isn't the same plugin (another name, or another main class and most classes different), with its comparison. Otherwise `null`. |
| `update` | object or null | The latest release, when it's newer than the jar. |
| `errors` | array of strings | Requests that failed. The result may be incomplete. |

`official` and `update` have the same fields:

| Field | Type | Meaning |
|---|---|---|
| `site` | string | `modrinth` or `hangar`. |
| `project` | string | The project's name there. |
| `url` | string | The project's page; for an update, the version's page. |
| `version`, `published` | string or null | The release's version number, and when it was published, as the site gives it. |
| `file`, `download` | string or null | The file's name and download address. |
| `diff` | object or null | A [comparison](#a-comparison). In `official`, the official file is `old` and the jar `new`, so `added` is what the jar adds; `null` when the bytes are the same. In `update`, the jar is `old` and the update `new`, and it's `null` when the release has no file for your server software. |

A re-zipped copy of an official release, checked with `--no-updates`:

```json
[
  {
    "file": "griefprevention-rezipped.jar",
    "sha256": "2d478b46dcc4405c9326171251c4e40dde086cf02fcb73a21aeb40616d5799a6",
    "plugin": "GriefPrevention",
    "version": "16.18.7",
    "status": "same-code",
    "official": {
      "site": "modrinth",
      "project": "GriefPrevention",
      "url": "https://modrinth.com/project/griefprevention",
      "version": "16.18.7",
      "published": "2026-03-05T16:46:32.405428Z",
      "file": "GriefPrevention.jar",
      "download": "https://cdn.modrinth.com/data/O4o4mKaq/versions/dGfCZHqk/GriefPrevention.jar",
      "diff": {
        "code": {
          "changed_classes": [],
          "added_classes": [],
          "removed_classes": [],
          "unchanged_classes": 127,
          "added_files": [],
          "removed_files": [],
          "changed_files": []
        },
        "same_code": true
      }
    },
    "update": null,
    "errors": []
  }
]
```

› real output: jarsnitch 0.4.0, `check griefprevention-rezipped.jar --no-updates --json`, Modrinth as of 15 Nov 2026, `diff` shortened to `code` and `same_code`

## Examples

A scan of the sample plugin, with 3 of its 11 findings. `corr.download-and-run` is reached, so it has a path: `onEnable` calls `AddonLoader.load`. `net.socket` is in code nothing calls, so its `reached` is `false` and its `path` is empty. The host in `plugin.yml` isn't code, so its `reached` is `null`.

```json
[
  {
    "schema_version": 1,
    "tool_version": "0.3.0",
    "file": {
      "name": "SampleShop-1.4.1.jar",
      "size": 5316,
      "sha256": "699600d2ac6a230cb881f8f12d0042ba13e3b8a30924d468643cefc967a70dcf"
    },
    "partial": false,
    "plugins": [
      {
        "platform": "bukkit",
        "descriptor": "plugin.yml",
        "name": "SampleShop",
        "version": "1.4.1",
        "main": "example.sampleshop.SampleShop",
        "bootstrapper": null,
        "loader": null,
        "authors": [
          "ExampleNewOwner"
        ],
        "website": "https://shop-plugins.example.net",
        "commands": [
          "shop"
        ],
        "permissions": [
          "sampleshop.use"
        ],
        "dependencies": [],
        "optional_dependencies": [
          "Vault"
        ],
        "folia_supported": false,
        "maven_libraries": []
      }
    ],
    "libraries": [],
    "nested": [],
    "obfuscation": {
      "score": 0,
      "short_names": 0.0,
      "default_package": 0.0,
      "encoded_strings": 0.0
    },
    "reachability": {
      "entry_points": [
        "example/sampleshop/SampleShop"
      ],
      "methods": 12,
      "reached_methods": 10,
      "dynamic": true
    },
    "findings": [
      {
        "rule": "corr.download-and-run",
        "severity": "high",
        "title": "Can download and run code from the internet",
        "why": "The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.",
        "legit_uses": "Dependency downloaders that fetch libraries from Maven repositories at startup. Check the hosts under str.url.",
        "value": null,
        "evidence": [
          {
            "location": "example/sampleshop/AddonLoader",
            "method": "load(Ljava/io/File;)V",
            "detail": "code.load: java/net/URLClassLoader.<init>",
            "reached": true
          },
          {
            "location": "example/sampleshop/AddonLoader",
            "method": "load(Ljava/io/File;)V",
            "detail": "str.url: https://cdn.example.invalid/addons/boost.jar",
            "reached": true
          }
        ],
        "evidence_total": 2,
        "reached": true,
        "path": [
          {
            "location": "example/sampleshop/SampleShop",
            "method": "onEnable()V",
            "via": "entry"
          },
          {
            "location": "example/sampleshop/AddonLoader",
            "method": "load(Ljava/io/File;)V",
            "via": "call"
          }
        ]
      },
      {
        "rule": "net.socket",
        "severity": "notable",
        "title": "Can open raw network connections",
        "why": "A raw socket can talk any protocol to any host, or listen for incoming connections on your server.",
        "legit_uses": "Vote listeners, query and RCON tools, proxies, database drivers, bundled HTTP clients.",
        "value": null,
        "evidence": [
          {
            "location": "example/sampleshop/libs/net/TcpClient",
            "method": "connect(Ljava/lang/String;I)Ljava/net/Socket;",
            "detail": "java/net/Socket.<init>",
            "reached": false
          }
        ],
        "evidence_total": 1,
        "reached": false,
        "path": []
      },
      {
        "rule": "str.url",
        "severity": "info",
        "title": "Contains web addresses",
        "why": "These are hosts the plugin may contact. Unfamiliar ones are worth a look.",
        "legit_uses": "Update checks, statistics, documentation links, APIs.",
        "value": "shop-plugins.example.net",
        "evidence": [
          {
            "location": "plugin.yml",
            "method": null,
            "detail": "https://shop-plugins.example.net",
            "reached": null
          }
        ],
        "evidence_total": 1,
        "reached": null,
        "path": []
      }
    ],
    "warnings": [],
    "stats": {
      "entries": 7,
      "classes": 6,
      "uncompressed_bytes": 6507
    },
    "disclaimer": "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee."
  }
]
```

› real output: jarsnitch 0.3.0, `scan SampleShop-1.4.1.jar --json`, 8 of the 11 findings left out

The sample update, without `old` and `new`, which hold each side's file, plugins, `partial` and warnings:

```json
{
  "schema_version": 1,
  "tool_version": "0.4.0",
  "obfuscation": {
    "old": 0,
    "new": 0
  },
  "added": [
    {
      "kind": "finding",
      "finding": {
        "rule": "corr.download-and-run",
        "severity": "high",
        "title": "Can download and run code from the internet",
        "why": "The jar can fetch code that isn't in this file and run it, so this report can't tell you what that code does.",
        "legit_uses": "Dependency downloaders that fetch libraries from Maven repositories at startup. Check the hosts under str.url.",
        "value": null,
        "evidence": [
          {
            "location": "example/sampleshop/AddonLoader",
            "method": "load(Ljava/io/File;)V",
            "detail": "code.load: java/net/URLClassLoader.<init>",
            "reached": true
          },
          {
            "location": "example/sampleshop/AddonLoader",
            "method": "load(Ljava/io/File;)V",
            "detail": "str.url: https://cdn.example.invalid/addons/boost.jar",
            "reached": true
          }
        ],
        "evidence_total": 2,
        "reached": true,
        "path": [
          {
            "location": "example/sampleshop/SampleShop",
            "method": "onEnable()V",
            "via": "entry"
          },
          {
            "location": "example/sampleshop/AddonLoader",
            "method": "load(Ljava/io/File;)V",
            "via": "call"
          }
        ]
      }
    },
    {
      "kind": "class",
      "value": "example/sampleshop/AddonLoader",
      "rules": [
        "code.load",
        "corr.download-and-run",
        "fs.write-archive"
      ],
      "severity": "high"
    },
    {
      "kind": "finding",
      "finding": {
        "rule": "str.url",
        "severity": "info",
        "title": "Contains web addresses",
        "why": "These are hosts the plugin may contact. Unfamiliar ones are worth a look.",
        "legit_uses": "Update checks, statistics, documentation links, APIs.",
        "value": "shop-plugins.example.net",
        "evidence": [
          {
            "location": "plugin.yml",
            "method": null,
            "detail": "https://shop-plugins.example.net",
            "reached": null
          }
        ],
        "evidence_total": 1,
        "reached": null,
        "path": []
      }
    }
  ],
  "removed": [
    {
      "kind": "finding",
      "finding": {
        "rule": "str.url",
        "severity": "info",
        "title": "Contains web addresses",
        "why": "These are hosts the plugin may contact. Unfamiliar ones are worth a look.",
        "legit_uses": "Update checks, statistics, documentation links, APIs.",
        "value": "sampleshop.example.com",
        "evidence": [
          {
            "location": "plugin.yml",
            "method": null,
            "detail": "https://sampleshop.example.com",
            "reached": null
          }
        ],
        "evidence_total": 1,
        "reached": null,
        "path": []
      }
    }
  ],
  "changed": [
    {
      "kind": "plugin-field",
      "key": "plugin.yml authors",
      "old": "ExampleDev",
      "new": "ExampleNewOwner"
    },
    {
      "kind": "plugin-field",
      "key": "plugin.yml version",
      "old": "1.4.0",
      "new": "1.4.1"
    },
    {
      "kind": "plugin-field",
      "key": "plugin.yml website",
      "old": "https://sampleshop.example.com",
      "new": "https://shop-plugins.example.net"
    }
  ],
  "code": {
    "changed_classes": [
      {
        "class": "example/sampleshop/SampleShop",
        "severity": "info",
        "rules_added": [],
        "header_changed": false,
        "methods_added": [],
        "methods_removed": [],
        "methods_changed": [
          "onEnable()V"
        ],
        "fields_added": [],
        "fields_removed": [],
        "fields_changed": [],
        "calls_added": [
          "example/sampleshop/AddonLoader.<init>",
          "example/sampleshop/AddonLoader.load",
          "example/sampleshop/ChatListener.<init>",
          "example/sampleshop/SampleShop.getDataFolder",
          "example/sampleshop/SampleShop.getServer",
          "org/bukkit/Server.getPluginManager",
          "org/bukkit/plugin/PluginManager.registerEvents"
        ],
        "duplicate": false
      }
    ],
    "added_classes": [
      "example/sampleshop/AddonLoader",
      "example/sampleshop/ChatListener"
    ],
    "removed_classes": [],
    "unchanged_classes": 3,
    "added_files": [],
    "removed_files": [],
    "changed_files": [
      "plugin.yml"
    ]
  },
  "same_code": false,
  "unchanged": [
    {
      "rule": "mc.console-command",
      "value": null
    },
    {
      "rule": "net.http",
      "value": null
    },
    {
      "rule": "net.socket",
      "value": null
    },
    {
      "rule": "str.url",
      "value": "api.example.invalid"
    }
  ],
  "warnings": [],
  "disclaimer": "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee."
}
```

› real output: jarsnitch 0.4.0, `diff SampleShop-1.4.0.jar SampleShop-1.4.1.jar --json`, `old`, `new` and 6 of the 9 added items left out

## Order and stability

- **Deterministic.** The same jar gives byte-identical JSON on every platform. There are no timestamps, paths always use `/`, and every list is sorted.
- **Findings** are sorted by severity (high first), then rule id, then value. Evidence is sorted by location, method and detail, without duplicates. A path's steps go from the entry point to the code. Unlike the text output, the JSON doesn't move findings that aren't called; filter on `reached` instead.
- **Stable.** Adding a field isn't a breaking change. Renaming or removing a field, or changing what it means, raises `schema_version`. Rule ids are never renamed or reused.
- **A check** reports what Modrinth and Hangar list when it runs, so the same jar can get another result later. Each `diff` in it carries its own `schema_version`.
- **Shareable.** Webhook and bot tokens are cut to their first 4 characters. A report still holds the file name, hashes, hosts and webhook ids, so read it before you post it.

## Recipes with jq

High findings across a plugins folder:

```bash
jarsnitch scan plugins/ --json | jq -r '.[] | .file.name as $f | .findings[] | select(.severity == "high") | "\($f)  \(.rule)"'
```

Every host the jars contain:

```bash
jarsnitch scan plugins/ --json | jq -r '[.[].findings[] | select(.rule == "str.url") | .value] | unique[]'
```

What an update adds, one line per finding:

```bash
jarsnitch diff old.jar new.jar --json | jq -r '.added[] | select(.kind == "finding") | .finding | "\(.severity)  \(.rule)  \(.value // "")"'
```

Jars whose code differs from the official file:

```bash
jarsnitch check plugins/ --json | jq -r '.[] | select(.status == "differs") | "\(.file)  \(.official.url)"'
```

Updates, with the version you run:

```bash
jarsnitch check plugins/ --json | jq -r '.[] | select(.update) | "\(.file)  \(.version) -> \(.update.version)"'
```

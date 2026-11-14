# Limits

jarsnitch reads what's in the file and never runs it. That keeps your server out of reach of the plugin, and it also sets what a report can show. This page lists both kinds of limit: what static reading can't see, and where a scan stops.

## What it can't catch

<!-- include: README.md#what-it-cant-catch -->

## Rule by rule

The exact limits from the [rules](../rules.md):

- **`net.http`** is decided per method. A method that opens a bundled resource and a remote URL loses the remote one.
- **`reflect.indirect`** resolves names written as string literals only. Names built at runtime aren't resolved.
- **`str.hidden-text`** decodes constant values only. Text that's XORed, encrypted, split across methods or computed at runtime isn't decoded.
- **`blob.embedded-code`** doesn't reassemble a payload split across several strings and joined at runtime.
- **`lib.log4j-vulnerable`** needs the bundled copy's `pom.properties`. A relocated copy without it is missed.
- **Combinations** need their parts in one class, or in one method and the methods it calls directly (`corr.hidden-loader`: anywhere in the jar). Parts joined through longer call chains, fields or callbacks are each listed on their own. See [Correlations](../rules.md#correlations).
- **`str.ip`** skips private, loopback and other reserved ranges, and numbers that are more likely versions or object ids. Well-known resolvers like `1.1.1.1` only show up with a port or in a URL.
- **`str.url`** treats a text file with more than 100 bare hostnames as a domain list and reports none of them, with a `text.host-list` warning.

## Scan limits

<!-- include: docs/architecture.md#limits-and-robustness -->

## What makes a report partial

A report is marked partial when a limit or a damaged entry stopped part of the scan. These warnings set it: `zip.limit`, `zip.unsupported` (encrypted entries or an unusual compression method), `zip.bad-entry`, `nested.too-deep` and `decode.limit`. Everything found before that point is still listed. A partial report is not a clean one: the part that wasn't checked could hold anything.

A file that isn't a zip at all isn't partial but unreadable: it has a `zip.unreadable` warning and no findings, and the command line exits with 1.

## How archives are read

- The zip's **central directory** is read the way the JVM's `JarFile` reads it: the end record is found from the back, zip64 is supported, and offsets are corrected when data is prepended to the zip, as shell-script launchers do.
- **Duplicate names** are kept as separate entries, and both are scanned.
- **Unsafe paths** (`../`, a leading `/`, a drive letter, backslashes) are flagged as `jar.unsafe-path` and scanned under a cleaned name. jarsnitch never extracts files.
- **Nested archives** are scanned up to 3 levels deep, and so are archives decoded from strings.

Archives built to look different to different zip readers may still fool it.

## Mods

Fabric, Quilt, Forge and NeoForge jars get their name and version read and the same bytecode rules, but no mod-specific checks. They have no plugin main class, so there's no call graph and nothing is marked not called.

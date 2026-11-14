# Getting started

jarsnitch runs in two places: a web page that reads the jar inside your browser tab, and a small command line tool. Both run the same engine and give the same findings.

## In your browser

1. Open <https://jarsnitch.mbaconz.workers.dev>.
2. Drop a plugin `.jar` on the page, or click the drop zone to choose one. No jar at hand? Press **Try a sample plugin**.
3. The report appears under the drop zone. **Download JSON** saves the same report as a file.

<!-- gif: site-scan | The jarsnitch page: SampleShop-1.4.1.jar is dropped on the Scan tab, the report shows 2 high, 4 notable and 5 info findings and a map of where they are in the jar, and clicking ChatListener on the map jumps to its finding's call path from onEnable to the matched code. -->

The page loads its own files once, including the analyzer compiled to WebAssembly, and makes no request after that. Your jar is read inside the tab and never uploaded. [Privacy](privacy.md) shows how to check that yourself.

› analyzer download: jarsnitch.wasm 0.2.1, about 216 KB with brotli · measured on the live site, 14 Nov 2026

The page is tested in the Chromium, Firefox and WebKit engines. Safari as Apple ships it hasn't been tested yet.

## Install the command line tool

<!-- include: README.md#install -->

- **Prebuilt targets:** Windows x86-64, macOS x86-64 and arm64, Linux x86-64 (statically linked, so it runs on old distributions) and Linux arm64.
- **`cargo install`** needs Rust 1.88 or newer.
- **Where it goes:** the installers put `jarsnitch` in `$CARGO_HOME/bin` (usually `~/.cargo/bin`). They don't update it later; run the installer again to get a new release.
- **No Java needed.** The binary reads jar files as data.

Check the install:

```out $ jarsnitch --version
jarsnitch 0.3.0
```

## Your first scan

The website's sample plugin, SampleShop, is a made-up plugin built for these docs. Its two versions are on the site: [SampleShop-1.4.0.jar](https://jarsnitch.mbaconz.workers.dev/samples/SampleShop-1.4.0.jar) and [SampleShop-1.4.1.jar](https://jarsnitch.mbaconz.workers.dev/samples/SampleShop-1.4.1.jar).

```out $ jarsnitch scan SampleShop-1.4.0.jar
== SampleShop-1.4.0.jar ==
SampleShop 1.4.0 · bukkit (plugin.yml)
sha256 62903a86fa08 · 4 classes · obfuscation 0/100
call graph from example/sampleshop/SampleShop: 6 of 8 methods reached

INFO     Can run server commands   [mc.console-command]
         Commands run as the console have full operator power.
         at example/sampleshop/ShopCommand.onCommand

INFO     Can connect to websites   [net.http]
         The plugin can send and receive data over the internet. The hosts found in the jar are listed under str.url.
         at example/sampleshop/UpdateChecker.check

INFO     Contains web addresses: api.example.invalid   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at example/sampleshop/UpdateChecker.check

INFO     Contains web addresses: sampleshop.example.com   [str.url]
         These are hosts the plugin may contact. Unfamiliar ones are worth a look.
         at plugin.yml

NOTABLE  Can open raw network connections   [net.socket]
         A raw socket can talk any protocol to any host, or listen for incoming connections on your server.
         at example/sampleshop/libs/net/TcpClient.connect
         not called from the plugin's entry points

0 high · 1 notable · 4 info · 1 not called from entry points

This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.
```

› real output: jarsnitch 0.3.0, Windows · exit code 0

The exit code is 0: the jar has no High finding. The one notable finding, a raw socket, sits in a bundled helper class that the plugin never calls, so it's listed last and marked. [Read a report](report.md) goes through every line.

Then compare it with its update:

```bash
jarsnitch diff SampleShop-1.4.0.jar SampleShop-1.4.1.jar
```

That one exits with 2, because the update adds two High findings. [Compare two versions](compare.md) shows the output.

## Run the website yourself

<!-- include: README.md#run-the-website-yourself -->

## Next

- [Scan a plugin](scan.md): one jar, several jars or a whole plugins folder.
- [Compare two versions](compare.md): what an update adds.
- [Rules](../rules.md): every rule, what triggers it and its common legitimate uses.

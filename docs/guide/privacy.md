# Privacy

Your jar never leaves your machine. The website reads it inside your browser tab and the command line tool reads it from disk. Neither sends it anywhere.

One feature uses the network, and only when you ask for it: [checking against official releases](check.md) sends the jar's hashes and its plugin name and version to Modrinth and Hangar.

## What the website loads

When you open the page, once:

- `index.html`, `style.css`, `site.js` and `app.js`, and the `favicon.svg` icon.
- `worker.js` and `engine.js`, which run the analyzer off the page's main thread.
- `jarsnitch.wasm`, the analyzer itself.
- The two sample jars, SampleShop 1.4.0 and 1.4.1, a few KB each, so the sample buttons work offline too.

› jarsnitch.wasm 0.2.1: about 216 KB over the network with brotli · measured on the live site, 14 Nov 2026

After that, nothing, unless you press **Check against official releases** ([below](#checking-against-official-releases)). Scanning, comparing and **Download JSON** all happen in the tab: the JSON file is built from memory and handed to your browser's own download. These docs are ordinary separate pages with no scripts.

## Checking against official releases

**Check against official releases** is the only thing that makes the page contact another site. It sends:

- to `api.modrinth.com`: each jar's SHA-512 hash, its plugin name and version, and the hashes of the official files it matched;
- to `hangar.papermc.io`: the SHA-256 hash of each server plugin Modrinth doesn't have;
- to `cdn.modrinth.com`: requests for the official files and updates to compare with, which are compared in the tab.

The requests carry no cookies and no referrer. Like any request, they show those sites your IP address and browser. Nothing is sent before you press the button, and nothing is kept after. The command line's `check` also sends plugin names and versions to `hangar.papermc.io` and downloads from Hangar's CDN; see [What it sends](check.md#what-it-sends).

## What it never does

- Upload a file, or any part of one. The check sends hashes, never file contents.
- Run analytics, set cookies or store anything in your browser.
- Load fonts, scripts, styles or images from another site.
- Run the plugin. Its bytes are read as data; no Java runs at all.

## The Content Security Policy

The page carries this policy, and the host sends it again as a header for every file:

```text
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self' https://api.modrinth.com https://cdn.modrinth.com https://hangar.papermc.io; style-src 'self'; img-src 'self' data: blob:; base-uri 'none'; form-action 'none'
```

| Directive | What it allows |
|---|---|
| `default-src 'none'` | Nothing, unless a rule below says otherwise. |
| `script-src 'self' 'wasm-unsafe-eval'` | Scripts from this site only, and compiling its WebAssembly. No inline scripts. |
| `worker-src 'self'` | The analyzer worker, from this site. |
| `connect-src 'self' https://api.modrinth.com https://cdn.modrinth.com https://hangar.papermc.io` | Requests to this site, which is how the page fetches the analyzer and the sample jars, and to Modrinth and Hangar, which only the check against official releases uses. No other site can receive anything from the page. |
| `style-src 'self'` | Stylesheets from this site only. No inline styles. |
| `img-src 'self' data: blob:` | Images from this site, or made in the page. |
| `base-uri 'none'`, `form-action 'none'` | No base URL rewriting and no form submissions. |

The header copy in `web/_headers` adds `frame-ancestors 'none'`, which stops other sites from framing the page. A `<meta>` tag can't carry that directive, and it doesn't cover the worker, so the header matters.

## Check it yourself

1. Open the site, then your browser's DevTools and its **Network** tab.
2. Reload, then clear the list.
3. Scan a jar and compare two. The list stays empty.
4. Turn on offline mode (in Chrome, the throttling menu's **Offline**) and scan again. It still works.
5. Go back online and press **Check against official releases**. The new requests go only to `api.modrinth.com`, `hangar.papermc.io` and `cdn.modrinth.com`; open one to see what it carries.

## Hosting

The site is a folder of static files served by Cloudflare. Like any web host, it sees the ordinary requests for those files when the page loads, such as your IP address and browser. It never sees your jars, because they're never sent.

You can host it yourself: build it with `scripts/build-web.sh` (or `.ps1`), serve the `web/` folder with any static server, and send the headers in `web/_headers` if your server supports them. See [Getting started](start.md#run-the-website-yourself).

## The command line tool

`jarsnitch scan` and `jarsnitch diff` read the files you name and write to your terminal; they never use the network. `jarsnitch check` is the only command that does. It sends what's listed in [What it sends](check.md#what-it-sends), with a `User-Agent` that names jarsnitch and its version, and says so on standard error first. There's no telemetry and no update check. Like the website, it never loads or runs the plugin.

## Before you share a report

A report is meant to be shareable: webhook and bot tokens are cut to their first 4 characters. It still contains the file name, its hashes, the hosts and IP addresses found in the code, and webhook ids. Read it before you post it.

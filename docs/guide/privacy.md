# Privacy

Your jar never leaves your machine. The website reads it inside your browser tab and the command line tool reads it from disk. Neither sends it, or anything about it, anywhere.

## What the website loads

When you open the page, once:

- `index.html`, `style.css`, `site.js` and `app.js`, and the `favicon.svg` icon.
- `worker.js` and `engine.js`, which run the analyzer off the page's main thread.
- `jarsnitch.wasm`, the analyzer itself.
- The two sample jars, SampleShop 1.4.0 and 1.4.1, a few KB each, so the sample buttons work offline too.

› jarsnitch.wasm 0.2.1: about 216 KB over the network with brotli · measured on the live site, 14 Nov 2026

After that, nothing. Scanning, comparing and **Download JSON** all happen in the tab: the JSON file is built from memory and handed to your browser's own download. These docs are ordinary separate pages with no scripts.

## What it never does

- Upload a file, or any part of one.
- Run analytics, set cookies or store anything in your browser.
- Load fonts, scripts, styles or images from another site.
- Run the plugin. Its bytes are read as data; no Java runs at all.

## The Content Security Policy

The page carries this policy, and the host sends it again as a header for every file:

```text
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self'; style-src 'self'; img-src 'self' data: blob:; base-uri 'none'; form-action 'none'
```

| Directive | What it allows |
|---|---|
| `default-src 'none'` | Nothing, unless a rule below says otherwise. |
| `script-src 'self' 'wasm-unsafe-eval'` | Scripts from this site only, and compiling its WebAssembly. No inline scripts. |
| `worker-src 'self'` | The analyzer worker, from this site. |
| `connect-src 'self'` | Requests to this site only. That's how the page fetches the analyzer and the sample jars, and it's why the page can't send data to anyone else. |
| `style-src 'self'` | Stylesheets from this site only. No inline styles. |
| `img-src 'self' data: blob:` | Images from this site, or made in the page. |
| `base-uri 'none'`, `form-action 'none'` | No base URL rewriting and no form submissions. |

The header copy in `web/_headers` adds `frame-ancestors 'none'`, which stops other sites from framing the page. A `<meta>` tag can't carry that directive, and it doesn't cover the worker, so the header matters.

## Check it yourself

1. Open the site, then your browser's DevTools and its **Network** tab.
2. Reload, then clear the list.
3. Scan a jar and compare two. The list stays empty.
4. Turn on offline mode (in Chrome, the throttling menu's **Offline**) and scan again. It still works.

## Hosting

The site is a folder of static files served by Cloudflare. Like any web host, it sees the ordinary requests for those files when the page loads, such as your IP address and browser. It never sees your jars, because they're never sent.

You can host it yourself: build it with `scripts/build-web.sh` (or `.ps1`), serve the `web/` folder with any static server, and send the headers in `web/_headers` if your server supports them. See [Getting started](start.md#run-the-website-yourself).

## The command line tool

`jarsnitch` reads the files you name and writes to your terminal. It has no network code, no telemetry and no update check. Like the website, it never loads or runs the plugin.

## Before you share a report

A report is meant to be shareable: webhook and bot tokens are cut to their first 4 characters. It still contains the file name, its hashes, the hosts and IP addresses found in the code, and webhook ids. Read it before you post it.

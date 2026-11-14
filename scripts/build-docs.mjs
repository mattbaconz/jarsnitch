// Builds the docs site: docs/guide/*.md, docs/*.md and CHANGELOG.md become
// web/docs/<slug>/index.html, styled by web/style.css plus docs/site/docs.css.
// No dependencies and no JavaScript on the pages.
//
//   node scripts/build-docs.mjs
//
// Markdown: headings, paragraphs, **bold**, *italic*, `code`, [links](x), <https://autolinks>,
// lists (one nested level, fenced code inside items), pipe tables, fenced code, > quotes, ---.
// HTML in the source is always escaped. A paragraph starting with "›" is a provenance caption.
// Directives are HTML comments, so GitHub hides them:
//   <!-- include: README.md#anchor -->   that section's text, or a whole file from its first ## heading
//   <!-- diagram: name -->               docs/diagrams/name.svg (or .html) in a <figure>, captioned by its <desc>
//   <!-- gif: name | alt text -->        docs/media/name.gif with name.png for reduced motion; a cli-* gif
//                                        is hidden on phones, where docs/media/name.txt (if any) shows instead
//   <!-- bars -->                        the next table (rule, severity, plugins, reached) as a bar chart
//   <!-- pages -->                       every docs page with its blurb
//
// The build fails on a broken internal link or anchor, any inline style, script or event
// handler in the output, and "malware", "safe" or "clean" outside the negations in ALLOWED.

import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const OUT = join(ROOT, "web/docs");
const REPO = "https://github.com/mattbaconz/jarsnitch";

// Each blurb is the page's meta description and its line in the docs index.
const PAGES = [
  { group: "Start", slug: "", title: "Docs", src: "docs/guide/index.md", blurb: "How to scan a plugin, compare two versions and read the report, on the website or the command line." },
  { group: "Start", slug: "start", title: "Getting started", src: "docs/guide/start.md", blurb: "Use the website, or install the command line tool and run your first scan." },
  { group: "Use", slug: "scan", title: "Scan a plugin", src: "docs/guide/scan.md", blurb: "One jar, several, or a whole plugins folder, on the website or with jarsnitch scan." },
  { group: "Use", slug: "compare", title: "Compare two versions", src: "docs/guide/compare.md", blurb: "See what an update adds before you install it, including a new author or website." },
  { group: "Use", slug: "report", title: "Read a report", src: "docs/guide/report.md", blurb: "What each part of a finding means, and every line of the website and command line reports." },
  { group: "Use", slug: "call-graph", title: "The call graph", src: "docs/guide/call-graph.md", blurb: "How calls are followed from the plugin's entry points, and what \u201cnot called\u201d means." },
  { group: "Reference", slug: "rules", title: "Rules", src: "docs/guide/rules.md", blurb: "Every rule: what it looks for, why it matters and its common legitimate uses." },
  { group: "Reference", slug: "cli", title: "Command line", src: "docs/guide/cli.md", blurb: "Commands, options, text output and exit codes for update scripts and CI." },
  { group: "Reference", slug: "json", title: "JSON format", src: "docs/guide/json.md", blurb: "The --json report and Download JSON, field by field, with jq recipes." },
  { group: "Reference", slug: "limits", title: "Limits", src: "docs/guide/limits.md", blurb: "What reading the bytecode can't show, and where a scan stops on huge or damaged jars." },
  { group: "About", slug: "privacy", title: "Privacy", src: "docs/guide/privacy.md", blurb: "Your jar never leaves your machine. What the website loads, and what it never sends." },
  { group: "About", slug: "faq", title: "FAQ", src: "docs/guide/faq.md", blurb: "Short answers to the questions people ask first, with links to the long ones." },
  { group: "About", slug: "benchmarks", title: "Benchmarks", src: "docs/benchmarks.md", blurb: "Speed and findings on Modrinth's 100 most-downloaded plugins, and how they were measured." },
  { group: "About", slug: "how-it-works", title: "How it works", src: "docs/architecture.md", blurb: "From zip to report: the parser, the rules, the call graph and the comparison." },
  { group: "About", slug: "changelog", title: "Changelog", src: "CHANGELOG.md", blurb: "What changed in each release." },
];

// The only places the verdict words may appear: negations and the fixed disclaimer.
const ALLOWED = [
  /a clean report isn't a guarantee/gi,
  /a partial report is not a clean one/gi,
  /never calls a plugin malware, safe or clean/gi,
  /whether a plugin is malware/gi,
  /known malware by name/gi,
  /ships no malware signatures/gi,
  /url-safe/gi,
];

const errors = [];
const fail = (msg) => errors.push(msg);
const read = (path) => readFileSync(join(ROOT, path), "utf8").replace(/\r\n/g, "\n");
const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const unesc = (s) => s.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, "&");
const text = (html) => unesc(html.replace(/<[^>]*>/g, ""));

/** GitHub's heading anchors: lower case, punctuation dropped, each space a hyphen. */
const slugify = (s) => s.toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, "").replace(/ /g, "-");

const pageBySrc = new Map(PAGES.map((p) => [p.src, p]));
const RULE_IDS = new Set([...read("docs/rules.md").matchAll(/^### `([a-z0-9.-]+)` · /gm)].map((m) => m[1]));

// Sections pulled in with "include" are linked to where they're published.
const included = new Map();
for (const p of PAGES) {
  for (const [, file, anchor] of read(p.src).matchAll(/<!-- include: ([^#\s]+)(?:#(\S+))? -->/g)) {
    included.set(anchor ? `${file}#${anchor}` : file, p);
  }
}

const pageUrl = (from, to) => (from.slug ? "../" : "") + (to.slug ? `${to.slug}/` : "") || "./";
const rootUrl = (page) => (page.slug ? "../../" : "../");

/** A markdown link target, resolved from the file it's written in. */
function href(raw, ctx) {
  if (/^[a-z]+:|^\/\//i.test(raw)) return raw;
  const [path, hash] = raw.split("#");
  if (!path) return `#${hash}`;
  const file = posix.normalize(posix.join(posix.dirname(ctx.src), path));
  // A whole published file wins over a section of it that's also shown elsewhere.
  const target = pageBySrc.get(file) || included.get(file) || (hash && included.get(`${file}#${hash}`));
  if (target) return pageUrl(ctx.page, target) + (hash ? `#${hash}` : "");
  if (!existsSync(join(ROOT, file))) {
    fail(`${ctx.src}: link to missing file ${raw}`);
    return raw;
  }
  if (file === "README.md") return REPO + (hash ? `#${hash}` : "");
  return `${REPO}/blob/main/${file}` + (hash ? `#${hash}` : "");
}

/** Inline markdown. Code spans first, so their contents are never formatted. */
function inline(s, ctx) {
  const held = [];
  const hold = (html) => `\u0000${held.push(html) - 1}\u0000`;
  s = s.replace(/(`+)(.+?)\1/g, (_, __, c) => hold(`<code>${esc(c.trim())}</code>`));
  s = esc(s);
  s = s.replace(/&lt;(https?:\/\/[^\s&]+)&gt;/g, (_, u) => hold(`<a href="${u}">${u}</a>`));
  s = s.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (_, t, u) => `<a href="${esc(href(unesc(u), ctx))}">${t}</a>`);
  s = s.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  s = s.replace(/(^|[^*\w])\*(?=\S)([^*]+?)\*(?!\*)/g, "$1<em>$2</em>");
  s = s.replace(/(^|[^\w])_(?=\S)([^_]+?)_(?!\w)/g, "$1<em>$2</em>");
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => held[i]);
}

/** A region's accessible name, numbered when it repeats on the page: "Speed", "Speed, 2". */
function regionName(ctx, name) {
  const n = (ctx.names.get(name) ?? 0) + 1;
  ctx.names.set(name, n);
  return n > 1 ? `${name}, ${n}` : name;
}

/** A table cell: severity words become badges, rule ids link to their rule. */
function cell(raw, ctx) {
  const t = raw.trim();
  if (/^(info|notable|high)$/.test(t)) return `<span class="sev sev-${t}">${t}</span>`;
  const rule = t.match(/^`([a-z0-9.-]+)`$/);
  if (rule && RULE_IDS.has(rule[1])) {
    const rules = PAGES.find((p) => p.slug === "rules");
    const to = ctx.page === rules ? "" : pageUrl(ctx.page, rules);
    return `<a class="rid" href="${to}#${rule[1]}"><code>${rule[1]}</code></a>`;
  }
  return inline(t, ctx);
}

// On phones each row stacks and the header row is hidden. A short value after the first
// column (a type, an exit code) shows its column name from data-label, and so do sentences
// when two columns hold them side by side (scan / diff); a lone sentence column reads on its
// own. A severity chip sits on the line of the cell before it (class "inl").
const SEV_CELL = /^(info|notable|high)$/;
const tdCell = (html, label, inl) => `<td${label ? ` data-label="${esc(label)}"` : ""}${inl ? ' class="inl"' : ""}>${html}</td>`;

const splitRow = (line) => line.trim().replace(/^\||\|$/g, "").split("|");

function table(rows, ctx) {
  const [head, , ...body] = rows.map(splitRow);
  const name = esc(regionName(ctx, ctx.heading || "Table"));
  const cols = head.map((_, k) => body.map((r) => (r[k] ?? "").trim()));
  const sev = cols.map((c) => c.every((v) => SEV_CELL.test(v)));
  const short = cols.map((c) => c.join("").length / c.length <= 30);
  const prose = short.filter((s, k) => k > 0 && !sev[k] && !s).length;
  const label = head.map((c, k) => (head.length > 2 && k > 0 && !sev[k] && (short[k] || prose > 1) ? text(inline(c.trim(), ctx)) : null));
  const inl = head.map((_, k) => (k > 0 && sev[k]) || !!sev[k + 1]);
  return `<div class="tbl" role="region" aria-label="${name}" tabindex="0"><table>
<thead><tr>${head.map((c) => `<th>${inline(c.trim(), ctx)}</th>`).join("")}</tr></thead>
<tbody>${body.map((r) => `<tr>${r.map((c, k) => tdCell(cell(c, ctx), label[k], inl[k])).join("")}</tr>`).join("\n")}</tbody>
</table></div>`;
}

/** 100 plugins as two rows of 50 squares, column by column: solid where an entry point reaches
 *  the code, hollow where only code nothing calls has it. web/index.html and the launch chart
 *  charts/can-do.png draw the same encoding; colours come from web/style.css (.cells). */
function cells(n, reached) {
  const at = (i, hollow) => `M${Math.floor(i / 2) * 2 + (hollow ? 0.15 : 0)} ${(i % 2) * 2 + (hollow ? 0.15 : 0)}${hollow ? "h1.3v1.3h-1.3z" : "h1.6v1.6h-1.6z"}`;
  let on = "", off = "";
  for (let i = 0; i < n; i++) i < reached ? (on += at(i, false)) : (off += at(i, true));
  return `<svg class="cells" viewBox="0 0 99.6 3.6" aria-hidden="true" focusable="false"><rect width="99.6" height="3.6" fill="url(#cell-bg)"/>` +
    `${on ? `<path class="c-on" d="${on}"/>` : ""}${off ? `<path class="c-off" d="${off}"/>` : ""}</svg>`;
}
const CELL_DEFS = `<svg class="defs" aria-hidden="true" focusable="false"><defs><pattern id="cell-bg" width="2" height="2" patternUnits="userSpaceOnUse"><rect class="c-bg" width="1.6" height="1.6"/></pattern></defs></svg>`;
const CELL_KEY = `<span><i class="on"></i>reached from an entry point</span><span><i></i>only in code nothing calls</span>`;

/** The benchmarks table (rule, severity, plugins, reached) as squares out of 100. */
function bars(rows, ctx) {
  const [, , ...body] = rows.map(splitRow);
  const items = body.map((r) => {
    const [rule, sev, n, reached] = r.map((c) => c.trim());
    const id = rule.replace(/`/g, "");
    if (!RULE_IDS.has(id) || !/^(info|notable|high)$/.test(sev) || !(+n >= +reached)) fail(`${ctx.src}: bars row "${r.join("|")}"`);
    return `<li class="sev-${sev}"><span class="b-l">${cell(rule, ctx)}<span class="sev sev-${sev}">${sev}</span></span>` +
      `<span class="b-r">${reached} reached</span><span class="b-n">${n}<span class="vh"> of 100 plugins</span></span>${cells(+n, +reached)}</li>`;
  });
  return `${CELL_DEFS}<ol class="dbars" aria-label="Plugins out of 100 with at least one finding of each rule, and how many of them in reached code">\n${items.join("\n")}\n</ol>
<p class="dbars-key cells-key">${CELL_KEY}</p>`;
}

/** Inlines docs/diagrams/<name>.svg; its <desc> becomes the visible caption. An optional
 *  <name>-narrow.svg is a stacked version shown instead on phones. A <name>.html is real
 *  page markup instead, captioned by the comment on its first line. */
function diagram(name) {
  const html = `docs/diagrams/${name}.html`;
  if (existsSync(join(ROOT, html))) {
    const src = read(html);
    const cap = src.match(/^<!--([\s\S]*?)-->\s*/);
    if (!cap) fail(`${html} needs a caption comment on its first line`);
    return `<figure class="dg dg-card dg-${name}">
${src.slice(cap?.[0].length ?? 0).trim()}
<figcaption>${esc(cap?.[1].trim() ?? "")}</figcaption>
</figure>`;
  }
  const svgs = [[name, "dg-w"], [`${name}-narrow`, "dg-n"]].filter(([f], k) => k === 0 || existsSync(join(ROOT, `docs/diagrams/${f}.svg`)));
  let caption = "";
  const parts = svgs.map(([file, cls]) => {
    const path = `docs/diagrams/${file}.svg`;
    if (!existsSync(join(ROOT, path))) return fail(`missing ${path}`), "";
    let svg = read(path).replace(/<\?xml[^>]*>\s*/, "").replace(/\s+xmlns="[^"]*"/, "");
    const desc = svg.match(/<desc>([\s\S]*?)<\/desc>/);
    if (file === name) caption = desc ? desc[1].trim() : (fail(`${path} needs a <desc>`), "");
    return svg.replace(/<desc>[\s\S]*?<\/desc>\s*/, "").replace("<svg", `<svg${svgs.length > 1 ? ` class="${cls}"` : ""} role="img" aria-labelledby="dg-${name}"`).trim();
  });
  return `<figure class="dg dg-${name}">\n${parts.join("\n")}\n<figcaption id="dg-${name}">${caption}</figcaption>\n</figure>`;
}

const media = new Set();
function gif(arg) {
  const [name, alt = ""] = arg.split("|").map((s) => s.trim());
  for (const ext of ["gif", "png"]) {
    const f = join(ROOT, `docs/media/${name}.${ext}`);
    if (!existsSync(f)) fail(`missing docs/media/${name}.${ext}`);
    if (statSync(f).size === 0) fail(`empty docs/media/${name}.${ext}`);
    media.add(`${name}.${ext}`);
  }
  if (!alt) fail(`gif ${name} needs alt text`);
  const png = readFileSync(join(ROOT, `docs/media/${name}.png`));
  const [w, h] = [png.readUInt32BE(16), png.readUInt32BE(20)]; // the PNG header; the gif has the same size
  const cli = name.startsWith("cli-");
  const txt = join(ROOT, `docs/media/${name}.txt`);
  return (ctx) => `<figure class="gif${cli ? " gif-cli" : ""}"><picture><source srcset="${ctx.page.slug ? "../" : ""}media/${name}.png" media="(prefers-reduced-motion: reduce)">` +
    `<img src="${ctx.page.slug ? "../" : ""}media/${name}.gif" alt="${esc(alt)}" width="${w}" height="${h}" loading="lazy"></picture></figure>` +
    (cli && existsSync(txt) ? codeBlock("out", null, readFileSync(txt, "utf8").replace(/\r\n/g, "\n").trimEnd(), ctx, "code gif-txt") : "");
}

function pagesGrid(ctx) {
  const groups = [...new Set(PAGES.map((p) => p.group))];
  return `<div class="pgrid">${groups.map((g) => `<section aria-label="${g}"><p class="label">${g}</p><ul>${PAGES.filter((p) => p.group === g && p.slug)
    .map((p) => `<li><a href="${pageUrl(ctx.page, p)}"><strong>${esc(p.title)}</strong><span>${esc(p.blurb)}</span></a></li>`).join("")}</ul></section>`).join("")}</div>`;
}

/** A section's own text: the lines between the heading with that slug and the next heading. */
function section(file, anchor) {
  const lines = read(file).split("\n");
  let fence = false;
  const heads = lines.map((l) => {
    if (/^\s*```/.test(l)) fence = !fence;
    const m = !fence && l.match(/^(#{1,4}) (.*)$/);
    return m ? { level: m[1].length, slug: slugify(text(inline(m[2], { src: file, page: PAGES[0] }))) } : null;
  });
  if (!anchor) return lines.slice(Math.max(0, heads.findIndex((h) => h?.level === 2))); // a whole file, from its first section
  const start = heads.findIndex((h) => h?.slug === anchor);
  if (start < 0) return fail(`include: no heading #${anchor} in ${file}`), [];
  const end = heads.findIndex((h, i) => i > start && h);
  return lines.slice(start + 1, end < 0 ? undefined : end);
}

/** A fenced block. "out" is real CLI output: severities, section lines and the command are coloured. */
function codeBlock(lang, label, src, ctx, cls = "code") {
  let body = esc(src);
  if (lang === "out") body = body.replace(/^(HIGH|NOTABLE|INFO)(?= )/gm, (s) => `<span class="o-${s.toLowerCase()}">${s}</span>`)
    .replace(/^(== .* ==|This update (?:ADDS|REMOVES):.*|Changes:|Warnings:|Unchanged:)/gm, '<span class="o-k">$1</span>')
    .replace(/^(\$ .*)$/gm, '<span class="o-cmd">$1</span>');
  const head = label ? `<div class="code-h"><span class="label">${esc(label)}</span></div>` : "";
  // Named by its label, else by the first command in it, else by the section it's in.
  const cmd = src.match(/^\$ (.+)$/m)?.[1] ?? (lang === "bash" || lang === "sh" ? src.split("\n")[0] : null);
  const name = esc(regionName(ctx, label || cmd || ctx.heading || "Code"));
  return `<div class="${cls}">${head}<pre tabindex="0" role="region" aria-label="${name}"><code>${body}</code></pre></div>`;
}

const isList = (l) => /^(\s*)([-*]|\d+\.) /.exec(l);
const isBlockStart = (l, next = "") =>
  /^(#{1,4} |```|>|---+\s*$|<!--)/.test(l.trimStart()) || isList(l) || (l.startsWith("|") && /^\|?\s*:?-{3}/.test(next));

/** Block markdown. ctx: { src, page, ids, toc, heading }. */
function render(lines, ctx) {
  const out = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const t = line.trim();
    if (!t) { i++; continue; }

    const dir = t.match(/^<!-- (include|diagram|gif|bars|pages)(?:: (.+?))? -->$/);
    if (dir) {
      i++;
      const [, kind, arg] = dir;
      if (kind === "include") {
        const [file, anchor] = arg.split("#");
        if (anchor && !ctx.ids.has(anchor)) { ctx.ids.add(anchor); out.push(`<span id="${anchor}"></span>`); }
        out.push(render(section(file, anchor), { ...ctx, src: file }));
      } else if (kind === "diagram") out.push(diagram(arg));
      else if (kind === "gif") out.push(gif(arg)(ctx));
      else if (kind === "pages") out.push(pagesGrid(ctx));
      else {
        while (i < lines.length && !lines[i].trim()) i++;
        const rows = [];
        while (i < lines.length && lines[i].startsWith("|")) rows.push(lines[i++]);
        if (rows.length < 3) fail(`${ctx.src}: <!-- bars --> needs a table after it`);
        out.push(bars(rows, ctx));
      }
      continue;
    }
    if (t.startsWith("<!--")) {
      while (i < lines.length && !lines[i].includes("-->")) i++;
      i++;
      continue;
    }

    const fence = line.match(/^(\s*)```\s*(\S*)\s*(.*)$/);
    if (fence) {
      const code = [];
      i++;
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) code.push(lines[i++].slice(fence[1].length));
      i++;
      out.push(codeBlock(fence[2], fence[3], code.join("\n"), ctx));
      continue;
    }

    const h = t.match(/^(#{1,4}) (.*)$/);
    if (h) {
      i++;
      const level = h[1].length;
      if (level === 1) continue; // the page title comes from PAGES
      const html = inline(h[2], ctx);
      let slug = slugify(text(html));
      for (let n = 1; ctx.ids.has(slug); n++) slug = `${slugify(text(html))}-${n}`;
      ctx.ids.add(slug);
      const rule = h[2].match(/^`([a-z0-9.-]+)` · (info|notable|high)$/);
      let inner = html;
      if (rule && RULE_IDS.has(rule[1]) && !ctx.ids.has(rule[1])) {
        ctx.ids.add(rule[1]);
        inner = `<code><span class="anchor" id="${rule[1]}"></span>${rule[1]}</code><span class="sev sev-${rule[2]}">${rule[2]}</span>`;
      }
      if (level <= 3) ctx.toc.push({ level, slug, html: rule ? `<code>${rule[1]}</code>` : html });
      ctx.heading = text(html);
      out.push(`<h${level} id="${slug}"${inner !== html ? ' class="rh"' : ""}>${inner}</h${level}>`);
      continue;
    }

    if (/^---+$/.test(t)) { out.push("<hr>"); i++; continue; }

    if (t.startsWith("|") && /^\|?\s*:?-{3}/.test((lines[i + 1] || "").trim())) {
      const rows = [];
      while (i < lines.length && lines[i].trim().startsWith("|")) rows.push(lines[i++]);
      out.push(table(rows, ctx));
      continue;
    }

    if (t.startsWith(">")) {
      const q = [];
      while (i < lines.length && lines[i].trim().startsWith(">")) q.push(lines[i++].trim().replace(/^> ?/, ""));
      out.push(`<blockquote>${render(q, ctx)}</blockquote>`);
      continue;
    }

    const li = isList(line);
    if (li) {
      const ordered = /\d/.test(li[2]);
      const items = [];
      while (i < lines.length) {
        const m = isList(lines[i]);
        if (m && m[1].length === li[1].length && /\d/.test(m[2]) === ordered) {
          items.push([lines[i].slice(m[0].length)]);
          i++;
          continue;
        }
        if (!lines[i].trim()) {
          let j = i;
          while (j < lines.length && !lines[j].trim()) j++;
          const next = lines[j] || "";
          const nm = isList(next);
          if (j < lines.length && (next.startsWith(" ".repeat(li[1].length + 2)) || (nm && nm[1].length === li[1].length && /\d/.test(nm[2]) === ordered))) {
            if (!nm || nm[1].length > li[1].length) items.at(-1).push("");
            i = j;
            continue;
          }
          break;
        }
        if (lines[i].startsWith(" ".repeat(li[1].length + 1)) || !isBlockStart(lines[i])) {
          items.at(-1).push(lines[i].replace(new RegExp(`^ {0,${li[1].length + (ordered ? 3 : 2)}}`), ""));
          i++;
          continue;
        }
        break;
      }
      const tag = ordered ? "ol" : "ul";
      const start = ordered && +li[2] !== 1 ? ` start="${+li[2]}"` : "";
      // Items are always tight: an item's first paragraph isn't wrapped in <p>.
      out.push(`<${tag}${start}>${items.map((body) => `<li>${render(body, { ...ctx }).replace(/^<p>([\s\S]*?)<\/p>/, "$1")}</li>`).join("\n")}</${tag}>`);
      continue;
    }

    const para = [];
    while (i < lines.length && lines[i].trim() && !(para.length && isBlockStart(lines[i], lines[i + 1]))) para.push(lines[i++].trim());
    const html = inline(para.join("\n"), ctx);
    out.push(html.startsWith("›") ? `<p class="cap">${html}</p>` : `<p>${html}</p>`);
  }
  return out.join("\n");
}

// The site's own header: the same CSP, logo and nav as web/index.html.
const index = read("web/index.html");
const CSP = index.match(/<meta http-equiv="Content-Security-Policy" content="([^"]+)">/)[1];
const LOCKUP = index.match(/<!--logo:lockup-->([\s\S]*?)<!--\/logo:lockup-->/)[1];
const VERSION = read("Cargo.toml").match(/^version = "([^"]+)"/m)[1];
const DISCLAIMER = "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.";

function shell(page, body, toc, prev, next) {
  const root = rootUrl(page);
  const docs = (p) => pageUrl(page, p);
  const top = [["Docs", PAGES[0]], ["Rules", PAGES.find((p) => p.slug === "rules")], ["CLI", PAGES.find((p) => p.slug === "cli")], ["Benchmarks", PAGES.find((p) => p.slug === "benchmarks")]];
  const nav = [...new Set(PAGES.map((p) => p.group))].map((g) => `<div><p class="label">${g}</p><ul>${PAGES.filter((p) => p.group === g)
    .map((p) => `<li><a href="${docs(p)}"${p === page ? ' aria-current="page"' : ""}>${esc(p.title)}</a></li>`).join("")}</ul></div>`).join("\n");
  const tocHtml = toc.length > 1
    ? `<aside class="otp" aria-label="On this page"><p class="label">On this page</p><ul>${toc.map((t) => `<li class="l${t.level}"><a href="#${t.slug}">${t.html}</a></li>`).join("")}</ul></aside>`
    : "";
  const pager = (p, k) => (p ? `<a class="pg-${k}" href="${docs(p)}"><span class="label">${k === "prev" ? "Previous" : "Next"}</span><strong>${esc(p.title)}</strong></a>` : "<span></span>");
  const title = page.slug ? `${page.title} · jarsnitch docs` : "jarsnitch docs";
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="${CSP}">
  <meta name="referrer" content="no-referrer">
  <meta name="color-scheme" content="dark light">
  <title>${esc(title)}</title>
  <meta name="description" content="${esc(page.blurb)}">
  <link rel="icon" href="${root}favicon.svg" type="image/svg+xml">
  <link rel="stylesheet" href="${root}style.css">
  <link rel="stylesheet" href="${page.slug ? "../" : ""}docs.css">
</head>
<body>
  <a class="skip" href="#main">Skip to content</a>
  <header class="bar">
    <div class="wrap">
      <a class="brand" href="${root}" aria-label="jarsnitch">${LOCKUP}</a>
      <nav class="nav" aria-label="Site">
        ${top.map(([t, p]) => `<a href="${docs(p)}"${p === page ? ' aria-current="page"' : t === "Docs" && !top.some(([, q]) => q === page) ? ' aria-current="true"' : ""}>${t}</a>`).join("\n        ")}
        <a href="${REPO}">GitHub</a>
      </nav>
      <span class="ver" title="jarsnitch version">v${VERSION}</span>
    </div>
  </header>

  <div class="wrap docs">
    <details class="dmenu">
      <summary><span class="label">Docs menu</span><span class="dmenu-cur">${esc(page.title)}</span></summary>
      <nav class="dnav-m" aria-label="Docs">${nav}</nav>
    </details>
    <nav class="dnav" aria-label="Docs">${nav}</nav>

    <main class="prose" id="main">
      <p class="crumbs label">${page.slug ? `<a href="${docs(PAGES[0])}">docs</a> / ${page.group.toLowerCase()} / ${esc(page.title.toLowerCase())}` : "docs"}</p>
      <h1>${esc(page.slug ? page.title : "jarsnitch docs")}</h1>
${body}
      <nav class="pager" aria-label="Previous and next page">${pager(prev, "prev")}${pager(next, "next")}</nav>
    </main>
    ${tocHtml}
  </div>

  <footer class="foot dfoot">
    <div class="wrap">
      <p class="foot-claim">${DISCLAIMER}</p>
      <p class="foot-note">jarsnitch reads the bytecode and never runs the plugin. Free and open source under MIT or Apache-2.0. <a href="${REPO}">Source on GitHub</a>.</p>
    </div>
  </footer>
</body>
</html>
`;
}

const built = new Map();
PAGES.forEach((page, n) => {
  const ctx = { src: page.src, page, ids: new Set(["main"]), names: new Map(), toc: [], heading: page.title };
  const body = render(read(page.src).split("\n"), ctx)
    .replace(/^<p>/, '<p class="lede">'); // the opening paragraph
  built.set(page, shell(page, body, ctx.toc, PAGES[n - 1], PAGES[n + 1]));
});

// Checks.
const pathOf = (page) => (page.slug ? `docs/${page.slug}/index.html` : "docs/index.html");
const idsOf = new Map([...built].map(([p, html]) => [pathOf(p), new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]))]));
const webFile = (rel) => {
  const f = join(ROOT, "web", rel);
  return rel.endsWith("/") || (existsSync(f) && statSync(f).isDirectory()) ? join(f, "index.html") : f;
};
for (const [page, html] of built) {
  const at = pathOf(page);
  for (const [re, what] of [[/\sstyle\s*=/i, "inline style"], [/<style/i, "style element"], [/<script/i, "script"], [/\son[a-z]+\s*=/i, "event handler"]]) {
    if (re.test(html)) fail(`${at}: ${what} in output`);
  }
  const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]);
  const dup = ids.find((id, k) => ids.indexOf(id) !== k);
  if (dup) fail(`${at}: duplicate id "${dup}"`);

  let words = text(html.replace(/<head>[\s\S]*<\/head>/, "")).replace(/\s+/g, " ");
  for (const re of ALLOWED) words = words.replace(re, "");
  for (const m of words.matchAll(/\b(malware|safe|clean)\b/gi)) fail(`${at}: "${m[1]}" outside a negation: …${words.slice(Math.max(0, m.index - 60), m.index + 40)}…`);

  for (const [, link] of html.matchAll(/\s(?:href|src|srcset)="([^"]+)"/g)) {
    if (/^[a-z]+:|^\/\//i.test(link)) continue;
    const [path, hash] = unesc(link).split("#");
    let local = path ? posix.normalize(posix.join(posix.dirname(at), path)) : at;
    if (local === "." || local === "./") local = "";
    if (local === "" || local.endsWith("/")) local += "index.html";
    const ids = idsOf.get(local);
    // Generated files are checked against their sources; everything else must exist in web/.
    const exists = ids || (local === "docs/docs.css" ? existsSync(join(ROOT, "docs/site/docs.css"))
      : local.startsWith("docs/media/") ? media.has(local.slice(11)) : existsSync(webFile(local)));
    if (!exists) fail(`${at}: broken link ${link}`);
    else if (hash && !ids) fail(`${at}: anchor into a page this build doesn't check: ${link}`);
    else if (hash && !ids.has(hash)) fail(`${at}: broken anchor ${link}`);
  }
}

if (errors.length) {
  console.error(`build-docs: ${errors.length} problem${errors.length === 1 ? "" : "s"}\n  ${errors.join("\n  ")}`);
  process.exit(1);
}

rmSync(OUT, { recursive: true, force: true });
for (const [page, html] of built) {
  const file = join(ROOT, "web", pathOf(page));
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, html);
}
copyFileSync(join(ROOT, "docs/site/docs.css"), join(OUT, "docs.css"));
if (media.size) mkdirSync(join(OUT, "media"), { recursive: true });
for (const m of media) copyFileSync(join(ROOT, "docs/media", m), join(OUT, "media", m));
console.log(`built ${built.size} docs pages in web/docs`);

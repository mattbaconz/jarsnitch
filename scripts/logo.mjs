// Draws the jarsnitch logo from one pixel grid: the jar with an eye, and the
// half-block wordmark. Writes the standalone SVGs, the plain-text logo, and the
// inline copies in web/index.html between <!--logo:NAME--> markers.
//
//   node scripts/logo.mjs

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

const root = new URL("..", import.meta.url);
const out = (path, text) => {
  mkdirSync(new URL(".", new URL(path, root)), { recursive: true });
  writeFileSync(new URL(path, root), text);
  console.log(`wrote ${path}`);
};

// The jar: 16 text cells by 8 lines, each cell two pixels tall
// (▀ top pixel, ▄ bottom, █ both). P is the pupil.
const JAR = [
  "  ████████████  ",
  "  ▄▄▄▄▄▄▄▄▄▄▄▄  ",
  "▄██████████████▄",
  "█████▀▀▀▀▀▀█████",
  "██▀   PPPP   ▀██",
  "██▄   PPPP   ▄██",
  "█████▄▄▄▄▄▄█████",
  " ▀████████████▀ ",
];

// Wordmark glyphs, 12 pixel rows each. # is ink.
const G = {
  j: [".##", ".##", "...", ".##", ".##", ".##", ".##", ".##", ".##", ".##", ".##", "##."],
  a: ["......", "......", "......", "#####.", "....##", "....##", ".#####", "##..##", "##..##", ".#####", "......", "......"],
  r: [".....", ".....", ".....", "####.", "##.##", "##...", "##...", "##...", "##...", "##...", ".....", "....."],
  s: ["......", "......", "......", ".#####", "##....", "##....", ".####.", "....##", "....##", "#####.", "......", "......"],
  n: ["......", "......", "......", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "......", "......"],
  i: ["##", "##", "..", "##", "##", "##", "##", "##", "##", "##", "..", ".."],
  t: [".....", ".##..", ".##..", "#####", ".##..", ".##..", ".##..", ".##..", ".##..", "..###", ".....", "....."],
  c: ["......", "......", "......", ".#####", "##....", "##....", "##....", "##....", "##....", ".#####", "......", "......"],
  h: ["##....", "##....", "##....", "#####.", "##..##", "##..##", "##..##", "##..##", "##..##", "##..##", "......", "......"],
};

const GAP = 2; // columns between the jar and the wordmark
const WORD_Y = 2; // the wordmark starts on the jar's second text line

/** Pixels as a Map "x,y" -> class: i ink, p pupil, e echo. */
function jar() {
  const px = new Map();
  JAR.forEach((line, row) =>
    [...line].forEach((ch, x) => {
      const cls = ch === "P" ? "p" : "i";
      if ("█P▀".includes(ch)) px.set(`${x},${row * 2}`, cls);
      if ("█P▄".includes(ch)) px.set(`${x},${row * 2 + 1}`, cls);
    }));
  return px;
}

function lockup() {
  const px = jar();
  let x0 = 16 + GAP;
  for (const [k, ch] of [..."jarsnitch"].entries()) {
    if (k) x0 += 1;
    G[ch].forEach((row, y) => [...row].forEach((c, x) => c === "#" && px.set(`${x0 + x},${WORD_Y + y}`, "i")));
    x0 += G[ch][0].length;
  }
  return px;
}

/** The drop echo: every pixel again, one column right and one text line down, where nothing is drawn. */
function withEcho(px) {
  const all = new Map(px);
  for (const k of px.keys()) {
    const [x, y] = k.split(",").map(Number);
    const at = `${x + 1},${y + 2}`;
    if (!px.has(at)) all.set(at, "e");
  }
  return all;
}

const size = (px) => {
  let w = 0, h = 0;
  for (const k of px.keys()) {
    const [x, y] = k.split(",").map(Number);
    w = Math.max(w, x + 1);
    h = Math.max(h, y + 1);
  }
  return [w, h];
};

/** One <g> per class, with horizontal runs of pixels merged into one rect. `fill` gives
 *  each group a presentation attribute, which a style-src CSP doesn't block. */
function groups(px, fill = {}, order = ["e", "i", "p"]) {
  const [w, h] = size(px);
  return order.map((cls) => {
    const rects = [];
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        if (px.get(`${x},${y}`) !== cls) continue;
        let n = 1;
        while (px.get(`${x + n},${y}`) === cls) n++;
        rects.push(`<rect x="${x}" y="${y}" width="${n}" height="1"/>`);
        x += n - 1;
      }
    }
    return rects.length ? `<g class="${cls}"${fill[cls] ? ` fill="${fill[cls]}"` : ""}>${rects.join("")}</g>` : "";
  }).join("");
}

// Light colours as attributes, so the file still draws when opened directly under the site's
// CSP (style-src 'self' blocks the <style> below); the <style> only adds the dark palette.
const LIGHT = { i: "#0e1310", e: "#b4beb2", p: "#2e7d32" };
const STYLE = "<style>@media (prefers-color-scheme:dark){.i{fill:#eef2ee}.e{fill:#48574d}.p{fill:#5fb865}}</style>";

function standalone(px, title) {
  const [w, h] = size(px);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w * 8}" height="${h * 8}" shape-rendering="crispEdges" role="img" aria-label="${title}"><title>${title}</title>${STYLE}${groups(px, LIGHT)}</svg>\n`;
}

function inline(px, cls) {
  const [w, h] = size(px);
  return `<svg class="${cls}" viewBox="0 0 ${w} ${h}" shape-rendering="crispEdges" aria-hidden="true" focusable="false">${groups(px)}</svg>`;
}

/** Back to text: two pixel rows per line, and C's ░ echo per text cell. */
function text(px, echo) {
  const [w, h] = size(px);
  const lines = [];
  for (let y = 0; y < h; y += 2) {
    let s = "";
    for (let x = 0; x < w; x++) {
      const t = px.has(`${x},${y}`), b = px.has(`${x},${y + 1}`);
      s += t && b ? "█" : t ? "▀" : b ? "▄" : " ";
    }
    lines.push([...s]);
  }
  if (echo) {
    lines.push([]);
    for (const l of lines) while (l.length <= w) l.push(" ");
    for (let y = lines.length - 1; y > 0; y--) {
      for (let x = w; x > 0; x--) {
        const src = lines[y - 1][x - 1];
        if (lines[y][x] === " " && src !== " ") lines[y][x] = "░";
      }
    }
  }
  return lines.map((l) => l.join("").trimEnd()).join("\n").replace(/\n+$/, "");
}

const big = lockup();
const mark = jar();

out("web/logo.svg", standalone(withEcho(big), "jarsnitch"));
out("web/logo-mark.svg", standalone(mark, "jarsnitch"));
out("web/favicon.svg", standalone(mark, "jarsnitch"));
out("docs/media/logo.svg", standalone(withEcho(big), "jarsnitch"));
out("docs/media/logo-mark.svg", standalone(mark, "jarsnitch"));
out(
  "docs/media/logo.txt",
  `# jarsnitch logo. Print ░ dim and the 4x2 pupil block in the accent green (#5fb865).\n${text(big, true)}\n\n` +
    `# Plain, without the echo (NO_COLOR).\n${text(big, false)}\n\n# Mark only.\n${text(mark, false)}\n`,
);

// Inline copies in the page, so CSS can color them and site.js can animate the big one.
const page = new URL("web/index.html", root);
let html = readFileSync(page, "utf8");
const inlines = { mark: inline(mark, "logo logo-mark"), lockup: inline(withEcho(big), "logo logo-lockup") };
for (const [name, svg] of Object.entries(inlines)) {
  html = html.replace(new RegExp(`(<!--logo:${name}-->)[\\s\\S]*?(<!--/logo:${name}-->)`, "g"), (_, a, b) => a + svg + b);
}
writeFileSync(page, html);
console.log(`updated web/index.html\n\n${text(big, true)}`);

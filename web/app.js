// Page logic: file picking, talking to the worker, rendering reports.
// Report text comes from the scanned jar, so it is only ever inserted as text.

import { selectTab } from "./site.js";
import { checkAll, sameCode, addsHigh, alarming } from "./official.js";

const status = document.getElementById("status");
const results = document.getElementById("results");
const SEVS = ["high", "notable", "info"];
const LABEL = { high: "High", notable: "Notable", info: "Info" };

/** Builds an element. Strings become text nodes, never HTML. */
function h(tag, props, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props ?? {})) {
    if (value == null || value === false) continue;
    if (key === "class") el.className = value;
    else if (key.startsWith("on")) el.addEventListener(key.slice(2), value);
    else el.setAttribute(key, value === true ? "" : String(value));
  }
  for (const child of children.flat(Infinity)) {
    if (child != null && child !== false) el.append(child);
  }
  return el;
}

/** Builds an SVG element; attributes only, so the page's style policy is never involved. */
function svg(tag, props, ...children) {
  const el = document.createElementNS("http://www.w3.org/2000/svg", tag);
  for (const [key, value] of Object.entries(props ?? {})) el.setAttribute(key, String(value));
  for (const child of children.flat(Infinity)) if (child) el.append(child);
  return el;
}

function setStatus(text, error = false) {
  status.textContent = text;
  status.classList.toggle("error", error);
}

// Worker. Started right away so the wasm is fetched during page load and never later.

let fatal = null;
let nextId = 0;
/** Bumped by every new scan or comparison, so an older one stops and never shows. */
let generation = 0;
/** Aborted with every new scan or comparison, so an official-release check still running stops. */
let checks = new AbortController();
function nextGeneration() {
  checks.abort();
  checks = new AbortController();
  return ++generation;
}
const pending = new Map();
const worker = new Worker(new URL("worker.js", import.meta.url), { type: "module" });

function fail(message) {
  fatal = location.protocol === "file:"
    ? "This page has to be opened through a web server, not as a file. See the README."
    : `The analyzer couldn't start: ${message}`;
  setStatus(fatal, true);
  for (const p of pending.values()) p.reject(new Error(fatal));
  pending.clear();
}

worker.onmessage = ({ data }) => {
  if (data.type === "fatal") return fail(data.message);
  if (data.type === "ready") return;
  const p = pending.get(data.id);
  pending.delete(data.id);
  if (!p) return;
  if (data.type === "result") p.resolve(data);
  else p.reject(new Error(data.message));
};
worker.onerror = (e) => fail(e.message || "the worker failed to load");

async function analyze(op, files) {
  if (fatal) throw new Error(fatal);
  const id = ++nextId;
  const loaded = await Promise.all(files.map(async (f) => ({ name: f.name, bytes: await f.arrayBuffer() })));
  const answer = new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
  worker.postMessage({ id, op, files: loaded }, loaded.map((f) => f.bytes));
  return { id, ...(await answer) };
}

/** Compares two files and resolves to the diff JSON. */
const diffFiles = async (a, b) => (await analyze("diff", [a, b])).result;

// Mode tabs. site.js handles clicks, arrow keys and the 1 / 2 shortcuts.

const tabs = [document.getElementById("tab-scan"), document.getElementById("tab-compare")];

// Files.

const zones = Object.fromEntries([...document.querySelectorAll(".drop")].map((z) => [z.dataset.slot, z]));
const pair = { old: null, new: null };

function showFile(slot, file) {
  zones[slot].querySelector(".drop-file").textContent = file ? `${file.name} · ${size(file.size)}` : "";
}

function pick(slot, file) {
  if (slot === "scan") {
    showFile("scan", file);
    return scan(file);
  }
  pair[slot] = file;
  showFile(slot, file);
  if (pair.old && pair.new) compare();
  else setStatus(`Now choose the ${pair.old ? "new" : "old"} version.`);
}

for (const [slot, zone] of Object.entries(zones)) {
  const input = zone.querySelector("input");
  input.addEventListener("change", () => {
    const files = [...input.files];
    input.value = "";
    if (slot === "scan") handle(files, zone);
    else if (files[0]) pick(slot, files[0]);
  });
}

document.getElementById("swap").addEventListener("click", () => {
  [pair.old, pair.new] = [pair.new, pair.old];
  showFile("old", pair.old);
  showFile("new", pair.new);
  if (pair.old && pair.new) compare();
  else if (pair.old || pair.new) setStatus(`Now choose the ${pair.old ? "new" : "old"} version.`);
});

const hasFiles = (e) => [...(e.dataTransfer?.types ?? [])].includes("Files");
document.addEventListener("dragover", (e) => {
  if (!hasFiles(e)) return;
  e.preventDefault();
  document.body.classList.add("dragging");
});
document.addEventListener("dragleave", (e) => {
  if (!e.relatedTarget) document.body.classList.remove("dragging");
});
document.addEventListener("drop", (e) => {
  if (!hasFiles(e)) return;
  e.preventDefault();
  document.body.classList.remove("dragging");
  const zone = e.target.closest?.(".drop");
  // Entries can only be read during the event. A dropped folder counts the jars directly in it.
  const entries = [...(e.dataTransfer.items ?? [])].map((i) => i.webkitGetAsEntry?.()).filter(Boolean);
  if (!entries.some((entry) => entry.isDirectory)) return handle([...e.dataTransfer.files], zone);
  setStatus("Reading the folder…");
  jarsIn(entries).then((files) => handle(files, null, true), (err) => setStatus(`Couldn't read that folder: ${err.message}`, true));
});

const fileOf = (entry) => new Promise((resolve, reject) => entry.file(resolve, reject));

/** Dropped files as they are, plus the .jar files directly in dropped folders, like the CLI. */
async function jarsIn(entries) {
  const files = [];
  for (const entry of entries) {
    if (entry.isFile) files.push(await fileOf(entry));
    if (!entry.isDirectory) continue;
    const reader = entry.createReader();
    for (let batch; (batch = await new Promise((resolve, reject) => reader.readEntries(resolve, reject))).length; ) {
      for (const child of batch) {
        if (child.isFile && /\.jar$/i.test(child.name)) files.push(await fileOf(child));
      }
    }
  }
  return files;
}

/** "Foo-1.9.jar" and "foo_2.0 (1).jar" share the stem "foo": two versions of one plugin. */
const stem = (name) => name.toLowerCase().replace(/\.(jar|zip)$/, "").replace(/\d.*$/, "").replace(/[^a-z]+$/, "");

/** One jar is scanned, two versions of one plugin are compared, anything else is scanned jar by jar. */
function handle(files, zone, folder = false) {
  if (!files.length) return setStatus(folder ? "There's no .jar file directly in that folder." : "There's no file to scan.", true);
  if (files.length === 1) {
    const slot = zone?.dataset.slot ?? "scan";
    if (slot === "scan") selectTab(tabs[0]);
    return pick(slot, files[0]);
  }
  // Older first when the names carry versions: Foo-1.9.jar before Foo-1.10.jar.
  files.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
  const comparing = !zones.old.closest(".panel").hidden;
  if (files.length === 2 && !folder && (comparing || (stem(files[0].name) && stem(files[0].name) === stem(files[1].name)))) {
    selectTab(tabs[1]);
    [pair.old, pair.new] = files;
    showFile("old", pair.old);
    showFile("new", pair.new);
    return compare();
  }
  selectTab(tabs[0]);
  zones.scan.querySelector(".drop-file").textContent = plural(files.length, "jar");
  return scanMany(files);
}

// Samples: a made-up plugin built from fixtures/ by scripts/build-web. Fetched with the page,
// a few KB each, so the buttons work offline too.
const SAMPLE_NOTE = "Sample: SampleShop is a made-up plugin. Its 1.4.1 update names a new author and adds a downloader and a hidden operator command.";
const samples = ["SampleShop-1.4.0.jar", "SampleShop-1.4.1.jar"].map((name) =>
  fetch(new URL(`samples/${name}`, import.meta.url))
    .then((res) => (res.ok ? res.blob() : null))
    .then((blob) => blob && new File([blob], name, { type: "application/java-archive" }), () => null));

for (const button of document.querySelectorAll("[data-sample]")) {
  button.addEventListener("click", async () => {
    const [old, update] = await Promise.all(samples);
    if (!old || !update) return setStatus("The sample couldn't be loaded. Reload the page and try again.", true);
    if (button.dataset.sample === "scan") {
      selectTab(tabs[0]);
      showFile("scan", update);
      return scan(update, SAMPLE_NOTE);
    }
    selectTab(tabs[1]);
    [pair.old, pair.new] = [old, update];
    showFile("old", old);
    showFile("new", update);
    compare(SAMPLE_NOTE);
  });
}

async function scan(file, note) {
  const gen = nextGeneration();
  setStatus(`Analyzing ${file.name}…`);
  try {
    const { result, ms } = await analyze("scan", [file]);
    if (gen !== generation) return;
    const c = counts(result.findings);
    if (unreadable(result.warnings)) setStatus(`${file.name} isn't a readable jar or zip.`, true);
    else setStatus(`Scanned ${file.name} in ${seconds(ms)}: ${c.high} high, ${c.notable} notable, ${c.info} info.`);
    show(withNote(renderReport(result, { ms, sample: !!note, file }), note));
  } catch (e) {
    if (gen === generation) setStatus(e.message, true);
  }
}

/** Scans jars one after another; a jar that fails doesn't stop the rest. */
async function scanMany(files) {
  const gen = nextGeneration();
  const start = performance.now();
  const scanned = [];
  results.replaceChildren();
  for (const [i, file] of files.entries()) {
    setStatus(`Scanning ${i + 1} of ${files.length}: ${file.name}…`);
    try {
      scanned.push({ file, report: (await analyze("scan", [file])).result });
    } catch (e) {
      if (fatal) return setStatus(fatal, true);
      scanned.push({ file, error: e.message });
    }
    if (gen !== generation) return;
  }
  const ms = performance.now() - start;
  const readable = scanned.filter((s) => s.report && !unreadable(s.report.warnings)).map((s) => counts(s.report.findings));
  const failed = scanned.length - readable.length;
  setStatus(
    `Scanned ${plural(files.length, "jar")} in ${seconds(ms)}: ` +
      `${readable.filter((c) => c.high).length} with high findings, ${readable.filter((c) => c.notable).length} with notable ones` +
      `${failed ? `, ${failed} unreadable` : ""}.`,
  );
  show(renderMany(scanned, ms));
}

async function compare(note) {
  const gen = nextGeneration();
  const { old: a, new: b } = pair;
  setStatus(`Comparing ${a.name} with ${b.name}…`);
  try {
    const { result, ms } = await analyze("diff", [a, b]);
    if (gen !== generation) return;
    const bad = [[a, result.old], [b, result.new]].find(([, side]) => unreadable(side.warnings));
    if (bad) {
      results.replaceChildren();
      return setStatus(`${bad[0].name} isn't a readable jar or zip, so there's nothing to compare.`, true);
    }
    const findings = result.added.filter((i) => i.kind === "finding").map((i) => i.finding);
    const c = counts(findings);
    setStatus(`Compared in ${seconds(ms)}. The update adds ${c.high} high, ${c.notable} notable and ${c.info} info findings.`);
    show(withNote(renderDiff(result, { ms, sample: !!note }), note));
  } catch (e) {
    if (gen === generation) setStatus(e.message, true);
  }
}

function show(node) {
  results.replaceChildren(node);
  node.querySelector("h2")?.focus({ preventScroll: false });
}

// Rendering. The order follows the CLI.

const counts = (findings) => {
  const c = { high: 0, notable: 0, info: 0 };
  for (const f of findings) c[f.severity]++;
  return c;
};
const size = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)} MB` : n >= 1e3 ? `${Math.round(n / 1e3)} KB` : plural(n, "byte"));
const seconds = (ms) => (ms < 1000 ? `${Math.max(1, Math.round(ms))} ms` : `${(ms / 1000).toFixed(1)} s`);
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const where = (e) => {
  const loc = e.location || "(jar)";
  return e.method ? `${loc}.${e.method.split("(")[0]}` : loc;
};
const sevBadge = (severity) => h("span", { class: `sev sev-${severity}` }, LABEL[severity]);
const unreadable = (warnings) => warnings.some((w) => w.code === "zip.unreadable" && !w.location);

/** Text that may wrap after a / or a . (class and package names), not in the middle of a word. */
const breakable = (text) => String(text).split(/(?<=[/.$])(?=.)/).flatMap((part, i) => (i ? [h("wbr"), part] : [part]));
const code = (text, cls) => h("code", { class: cls }, breakable(text));

/** Rule ids, each linking to its entry in the rule docs. */
const ruleChip = (rules) =>
  h("code", { class: "rule" }, [rules].flat().map((r, i) => [i ? ", " : null, h("a", { href: `docs/rules/#${encodeURIComponent(r)}` }, r)]));

const units = (n) => "■".repeat(Math.min(n, 10)) + (n > 10 ? "+" : "");

function countList(c, suffix = "") {
  return h("ul", { class: "counts", "aria-label": `Findings${suffix} by severity` },
    SEVS.map((s) => h("li", { class: `sev-${s}${c[s] ? "" : " zero"}` },
      h("b", {}, String(c[s])),
      h("span", { class: "c-label" }, `${s}${suffix}`),
      h("span", { class: "units", "aria-hidden": "true" }, units(c[s])))));
}

function downloadButton(fileName, data) {
  return h("button", {
    type: "button",
    class: "btn",
    onclick: () => {
      const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
      const a = h("a", { href: url, download: fileName });
      document.body.append(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    },
  }, "Download JSON");
}

const legend = () =>
  h("div", { class: "legend" },
    h("p", { class: "label" }, "Reading this report"),
    h("dl", {}, [["high", "gives a jar a lot of control"], ["notable", "a powerful capability"], ["info", "common"]]
      .map(([s, text]) => h("div", {}, h("dt", {}, sevBadge(s)), h("dd", {}, text)))),
    h("p", { class: "legend-ask" }, "Ask whether each finding fits what the plugin says it does."));

const caveat = () => h("p", { class: "caveat" }, "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.");

const prov = (version, ms, sample) =>
  [version ? `jarsnitch ${version}` : "jarsnitch", "WebAssembly in this tab", ms != null ? seconds(ms) : null, sample ? "sample jars" : null].filter(Boolean).join(" · ");

/**
 * A report: a rail with the title, counts, download, contents and legend, then the sections.
 * A report nested in the multi-jar list gets a plain header instead of the rail.
 */
function frame(o, main) {
  const head = h("header", { class: "report-head" },
    o.label ? h("p", { class: "label" }, o.label) : null,
    h(o.nested ? "h4" : "h2", { tabindex: "-1", class: "report-title" }, o.title),
    h("p", { class: Array.isArray(o.meta) ? "meta seps" : "meta" }, Array.isArray(o.meta) ? h("span", {}, o.meta.map((m) => h("span", {}, m))) : o.meta),
    h("div", { class: "head-row" }, o.counts, o.download));
  const toc = o.nested ? null : o.toc?.filter(Boolean);
  const rail = h(o.nested ? "div" : "aside", { class: "rail", "aria-label": o.nested ? null : "Report summary" },
    head,
    toc?.length
      ? h("nav", { class: "toc", "aria-label": "In this report", onclick: openTarget },
          h("p", { class: "label" }, "In this report"),
          h("ul", {}, toc.map(([id, text, n]) => h("li", {}, h("a", { href: `#${id}` }, text, n != null ? h("span", {}, String(n)) : null)))))
      : null,
    o.nested || o.bad ? null : legend(),
    o.prov ? h("p", { class: "prov" }, o.prov) : null);
  return h("article", { class: o.nested ? "report nested" : "report" },
    rail,
    h("div", { class: "report-main" }, main, o.nested ? null : caveat()));
}

/** A contents link to a closed fold opens it. */
function openTarget(e) {
  const id = e.target.closest("a")?.hash.slice(1);
  const target = id && document.getElementById(id);
  if (target?.tagName === "DETAILS") target.open = true;
}

/** Heading tags: a report nested in the jar list sits two levels lower. */
const hx = (n, nested) => `h${n + (nested ? 2 : 0)}`;

function findingCard(f, ctx, id) {
  const title = f.value ? [f.title, ": ", code(f.value)] : f.title;
  const t = trail(f, ctx.jar);
  return h("article", { class: `finding sev-${f.severity}`, id, tabindex: id ? "-1" : null },
    h("div", { class: "finding-head" }, sevBadge(f.severity), h(hx(4, ctx.nested), {}, title), ruleChip(f.rule)),
    h("p", { class: "why" }, f.why),
    t.node,
    legit(f),
    morePlaces(f, t.shown));
}

const legit = (f) => h("p", { class: "legit" }, h("span", { class: "k" }, "Common legitimate uses"), h("span", { class: "vh" }, ": "), f.legit_uses);
const notCalled = () => h("p", { class: "unreached" }, "Not called from the plugin's entry points.");

// The trail: from the jar, through the call path the engine found, to the code that matched.

const VIA = {
  entry: ["entry", "The server starts the plugin here: its main class, or a class it names."],
  call: ["calls", "The step above calls this, directly or through an interface or superclass."],
  callback: ["callback", "The server, a library or the JVM calls this on an object the step above creates: an event handler, a command, run(), toString()."],
  "static-init": ["static init", "The step above uses the class, which runs its static initializer."],
  reflection: ["reflection", "The step above names the class for reflection, or lists its methods."],
};
const PRIM = { B: "byte", C: "char", D: "double", F: "float", I: "int", J: "long", S: "short", Z: "boolean" };

/** "(Ljava/io/File;[I)V" → "File, int[]". */
function params(desc) {
  const inside = /^\(([^)]*)\)/.exec(desc)?.[1] ?? "";
  const out = [];
  let dims = 0;
  for (let i = 0; i < inside.length; i++) {
    const c = inside[i];
    if (c === "[") {
      dims++;
      continue;
    }
    let t = PRIM[c] ?? c;
    if (c === "L") {
      const end = inside.indexOf(";", i);
      t = inside.slice(i + 1, end < 0 ? undefined : end).split("/").pop();
      i = end < 0 ? inside.length : end;
    }
    out.push(t + "[]".repeat(dims));
    dims = 0;
  }
  return out.join(", ");
}

/** A class (and method) as package, then class, then method, so the eye finds the class. */
function member(location, method) {
  const cut = location.lastIndexOf("/") + 1;
  const pkg = location.slice(0, cut);
  const cls = location.slice(cut);
  const at = method ? method.indexOf("(") : -1;
  const name = method ? method.slice(0, at < 0 ? undefined : at) : null;
  const args = at < 0 ? "" : params(method.slice(at));
  return h("code", {},
    pkg ? [h("span", { class: "pk" }, breakable(pkg)), h("wbr")] : null,
    name === "<init>" ? ["new ", cls, h("wbr"), h("span", { class: "m" }, `(${args})`)] : cls,
    name && name !== "<init>" ? h("span", { class: "m" }, name === "<clinit>" ? ".<clinit>" : [`.${name}`, h("wbr"), `(${args})`]) : null);
}

/** A matched call or text; a combination's part names its rule first. */
function matched(detail) {
  const part = /^([a-z]+\.[a-z-]+): ([\s\S]*)$/.exec(detail);
  return h("code", {}, part ? [ruleChip(part[1]), breakable(part[2])] : breakable(detail));
}

/** A trail row; the explanation is on hover and, for screen readers, in the label. */
const row = (cls, k, title, body) =>
  h("li", { class: cls }, h("span", { class: "t-k", title }, k, title ? h("span", { class: "vh" }, `: ${title}`) : null), body);

/** Steps of a long path: the first two and the last four, with a button for the rest. */
function steps(path) {
  const rows = path.map((s, i) => row("t-step t-on", i === 0 ? VIA.entry[0] : VIA[s.via]?.[0] ?? s.via, i === 0 ? VIA.entry[1] : VIA[s.via]?.[1], member(s.location, s.method)));
  if (rows.length <= 7) return rows;
  const hidden = rows.slice(2, -4);
  // Focus moves to the first revealed step, not to the page, when the button goes away.
  const reveal = () => {
    hidden[0].tabIndex = -1;
    more.replaceWith(...hidden);
    hidden[0].focus();
  };
  const more = h("li", { class: "t-more" }, h("span", { class: "t-k" }, ""),
    h("button", { type: "button", onclick: reveal }, `${plural(hidden.length, "more step")}`));
  return [...rows.slice(0, 2), more, ...rows.slice(-4)];
}

/**
 * How the plugin gets to a finding: the call path from an entry point when the engine found
 * one, or else the place in the jar, from the jar down, with a break when nothing the entry
 * points run calls the code. Then the matched calls or text at that place. Nodes follow the
 * jar map: squares for code the call graph covers, solid when reached and hollow when not;
 * circles for what it doesn't cover. Returns the list and the evidence it shows.
 */
function trail(f, jar) {
  const target = f.path.at(-1);
  const first = f.evidence.find((e) => e.reached === true) ?? f.evidence[0];
  if (!first) return { node: null, shown: [] };
  const at = target ?? first;
  const here = f.evidence.filter((e) => e.location === at.location && (e.method == null || e.method === (target ? target.method : first.method)));
  const shown = (here.length ? here : [first]).slice(0, 4);
  const state = f.path.length || first.reached === true ? "t-on" : first.reached === false ? "t-off" : "t-na";
  const rows = [];
  if (f.path.length) {
    rows.push(...steps(f.path));
  } else {
    rows.push(row("t-jar t-na", "jar", null, h("code", {}, breakable(jar))));
    if (first.reached === false) rows.push(h("li", { class: "t-gap" }, h("span", { class: "t-k" }, "no caller"), h("span", {}, "Nothing the entry points run calls this.")));
    // Locations nest: `lib.jar!/a/B` is a class in a bundled jar, `a/B!embedded-1!/c/D` a class
    // decoded from text stored in a/B. Evidence about the jar as a whole has no location.
    const parts = first.location ? first.location.split("!/") : [];
    parts.forEach((p, i) => {
      const last = i === parts.length - 1;
      const blob = /^(.*)!(embedded-\d+)$/.exec(p);
      if (blob) {
        rows.push(row("t-step t-na", "class", null, h("code", {}, breakable(blob[1]))));
        rows.push(row("t-step t-na", "embedded", "Code stored as encoded text in the class above, decoded and scanned.", h("code", {}, blob[2])));
        return;
      }
      const kind = !last ? "nested jar" : first.method || !/\.[A-Za-z0-9]+$/.test(p) ? "class" : "file";
      rows.push(row(`t-step ${last ? state : "t-na"}`, kind, null, last && first.method ? member(p, first.method) : h("code", {}, breakable(p))));
    });
  }
  rows.push(...shown.map((e) => row(`t-hit ${state}`, "match", null, matched(e.detail))));
  const label = f.path.length ? "How the plugin reaches this code" : first.reached === false ? "Where this code is, with no call path to it" : "Where this is in the jar";
  return { node: h("ol", { class: "trail", "aria-label": label }, rows), shown };
}

/** The finding's other places, when there are more than the trail shows. */
function morePlaces(f, shown) {
  if (f.evidence_total <= shown.length) return null;
  const line = (e) => [code(where(e)), e.detail ? h("span", { class: "detail" }, breakable(e.detail)) : null, e.reached === false ? h("span", { class: "detail" }, "not called") : null];
  const hidden = f.evidence_total - f.evidence.length;
  return h("details", { class: "where" },
    h("summary", {}, `All ${plural(f.evidence_total, "place")} it was found`),
    h("ul", {}, f.evidence.map((e) => h("li", {}, line(e)))),
    hidden > 0 ? h("p", { class: "muted" }, `${hidden} more not listed.`) : null);
}

/**
 * Cards for findings in report order, with code the entry points don't call last;
 * findings that share a rule (hosts, say) share a card.
 */
function cardGroups(findings) {
  const groups = [];
  const called = findings.filter((f) => f.reached !== false);
  for (const f of [...called, ...findings.filter((f) => f.reached === false)]) {
    const last = groups.at(-1);
    if (last?.[0].rule === f.rule && (last[0].reached === false) === (f.reached === false)) last.push(f);
    else groups.push([f]);
  }
  return groups;
}

/** Cards for groups of findings; `tag` makes their ids, which the jar map links to. */
function findingCards(groups, ctx, tag = "f") {
  return groups.map((g, i) => {
    const id = ctx.idp ? `${ctx.idp}${tag}${i}` : null;
    return g.length === 1 ? findingCard(g[0], ctx, id) : groupCard(g, ctx, id);
  });
}

function groupCard(group, ctx, id) {
  const nested = ctx.nested;
  const f = group[0];
  const row = (x) => h("li", {}, code(x.value ?? "(no value)"), x.evidence[0] ? h("span", { class: "detail" }, "in ", breakable(where(x.evidence[0])), x.evidence_total > 1 ? ` and ${plural(x.evidence_total - 1, "more place")}` : "") : null);
  const shown = 8;
  return h("article", { class: `finding sev-${f.severity}`, id, tabindex: id ? "-1" : null },
    h("div", { class: "finding-head" }, sevBadge(f.severity), h(hx(4, nested), {}, `${f.title} (${group.length})`), ruleChip(f.rule)),
    h("p", { class: "why" }, f.why),
    legit(f),
    h("ul", { class: "values" }, group.slice(0, shown).map(row)),
    group.length > shown
      ? h("details", { class: "where" }, h("summary", {}, `Show ${group.length - shown} more`), h("ul", { class: "values" }, group.slice(shown).map(row)))
      : null,
    f.reached === false ? notCalled() : null);
}

/** A collapsed section whose content is only built when it's opened. */
function fold(summary, build, { open = false, cls = "fold", id, heading } = {}) {
  const d = h("details", { class: cls, open, id }, h("summary", {}, heading ? h(heading, { class: "fold-h" }, summary) : summary));
  const fill = () => {
    if (d.dataset.filled) return;
    d.dataset.filled = "1";
    d.append(...[build()].flat());
  };
  if (open) fill();
  else d.addEventListener("toggle", fill);
  d.fill = fill;
  return d;
}

/** A report section; `n` is shown next to the title without being part of it. */
const section = (cls, id, title, n, body, nested) =>
  h("section", { class: `section ${cls}`.trim(), id }, h(hx(3, nested), { "data-n": n }, title), body);

function findingsSection(findings, reach, id, ctx) {
  const important = cardGroups(findings.filter((f) => f.severity !== "info"));
  const info = cardGroups(findings.filter((f) => f.severity === "info"));
  const infoCount = findings.filter((f) => f.severity === "info").length;
  const notCalledCount = findings.filter((f) => f.reached === false).length;
  return section("findings", id, "Findings", findings.length ? plural(findings.length, "finding") : null,
    findings.length === 0
      ? h("p", { class: "empty" }, "jarsnitch found nothing from its rule list in this jar.")
      : [
          notCalledCount && reach?.dynamic
            ? h("p", { class: "muted" }, `${plural(notCalledCount, "finding is", "findings are")} in code the plugin's entry points don't call, but the plugin also loads code at runtime, so that code may still run.`)
            : null,
          important.length ? h("div", { class: "list" }, findingCards(important, ctx)) : h("p", { class: "empty" }, "No high or notable findings."),
          infoCount ? fold(`Info findings (${infoCount})`, () => h("div", { class: "list" }, findingCards(info, ctx, "i")), { cls: "fold info", id: ctx.idp ? `${ctx.idp}info` : null }) : null,
        ], ctx.nested);
}

/** The card a finding is on, for links from the jar map. */
function cardIds(findings, idp) {
  const ids = new Map();
  for (const [tag, list] of [["f", findings.filter((f) => f.severity !== "info")], ["i", findings.filter((f) => f.severity === "info")]]) {
    cardGroups(list).forEach((g, i) => g.forEach((f) => ids.set(f, `${idp}${tag}${i}`)));
  }
  return ids;
}

/** Follows a jar map link, opening the info fold first when the card is inside it. */
function goToCard(e, idp) {
  const a = e.target.closest("a[href^='#']");
  if (!a) return;
  const id = a.hash.slice(1);
  let card = document.getElementById(id);
  const info = document.getElementById(`${idp}info`);
  // The fold builds its cards when it first opens, and hides them while it's closed.
  if (info && (!card || info.contains(card))) {
    info.open = true;
    info.fill();
    card = document.getElementById(id);
  }
  if (!card) return;
  e.preventDefault();
  card.scrollIntoView({ block: "start" });
  card.focus({ preventScroll: true });
}

const SEV_RANK = { high: 3, notable: 2, info: 1 };
const LETTER = { high: "H", notable: "N", info: "I" };
const MAP_ROWS = 16;
/** Reach of the code at a place: reached, not called, or outside the call graph. */
const REACH = {
  on: ["", "reached from an entry point"],
  off: [", not called", "nothing the entry points run calls it"],
  na: [", not traced", "not traced: a file, a nested jar or decoded code"],
};

/**
 * The jar map: every class or file with findings, by package, one mark per rule with the
 * severity's letter. A square is code the call graph covers, solid when the plugin's entry
 * points reach it and hollow when nothing they run calls it; a circle is a place it doesn't
 * cover. The trail draws its nodes the same way.
 */
function jarMap(r, id, ctx) {
  const places = new Map();
  const ids = ctx.idp ? cardIds(r.findings, ctx.idp) : new Map();
  for (const f of r.findings) {
    for (const e of f.evidence) {
      if (!e.location) continue;
      const p = places.get(e.location) ?? { rules: new Map(), card: null, top: 0 };
      const was = p.rules.get(f.rule)?.reach;
      const now = e.reached === true ? "on" : e.reached === false ? "off" : "na";
      p.rules.set(f.rule, { severity: f.severity, reach: [was, now].includes("on") ? "on" : [was, now].includes("na") ? "na" : "off" });
      if (SEV_RANK[f.severity] > p.top) {
        p.top = SEV_RANK[f.severity];
        p.card = ids.get(f) ?? null;
      }
      places.set(e.location, p);
    }
  }
  if (!places.size) return null;
  let list = [...places.entries()];
  const dropped = Math.max(0, list.length - MAP_ROWS);
  if (dropped) list = list.sort(([a, x], [b, y]) => y.top - x.top || y.rules.size - x.rules.size || a.localeCompare(b)).slice(0, MAP_ROWS);
  const byPkg = new Map();
  for (const [loc, p] of list) {
    const cut = loc.lastIndexOf("/") + 1;
    const pkg = loc.slice(0, cut);
    byPkg.set(pkg, [...(byPkg.get(pkg) ?? []), [loc.slice(cut), p]]);
  }
  const seen = new Set();
  const rows = [];
  for (const pkg of [...byPkg.keys()].sort()) {
    const items = byPkg.get(pkg).sort(([a], [b]) => a.localeCompare(b));
    // Files at the top of the jar hang from the jar itself.
    rows.push(h("li", { class: "j-pkg" }, h("span", {}, breakable(pkg || r.file.name))));
    items.forEach(([name, p], k) => {
      const rules = [...p.rules.entries()].sort(([a, x], [b, y]) => SEV_RANK[y.severity] - SEV_RANK[x.severity] || a.localeCompare(b));
      const says = rules.map(([rule, x]) => `${LABEL[x.severity]} ${rule}${REACH[x.reach][0]}`).join("; ");
      for (const [, x] of rules) seen.add(x.reach);
      rows.push(h("li", { class: k === items.length - 1 ? "j-last" : null },
        h("span", { class: "j-name" }, p.card ? h("a", { href: `#${p.card}` }, breakable(name)) : breakable(name)),
        h("span", { class: "sqs", title: says },
          h("span", { class: "vh" }, `${says}.`),
          rules.map(([, x]) => h("i", { class: `sq sq-${x.reach} sev-${x.severity}`, "aria-hidden": "true" }, LETTER[x.severity]))),
        h("span", { class: "j-rules", "aria-hidden": "true" }, rules.map(([rule]) => rule).join(" · "))));
    });
  }
  const reach = r.reachability;
  const pct = reach && reach.methods ? (100 * reach.reached_methods) / reach.methods : 0;
  return section("map", id, "Where in the jar", plural(places.size, "place"), [
    reach
      ? h("div", { class: "meter" },
          h("span", {}, `Call graph: ${reach.reached_methods} of ${reach.methods} methods reached from ${plural(reach.entry_points.length, "entry point")}`),
          svg("svg", { viewBox: "0 0 100 6", preserveAspectRatio: "none", "aria-hidden": "true" },
            svg("rect", { class: "m-bg", width: 100, height: 6 }),
            svg("rect", { class: "m-on", width: pct.toFixed(2), height: 6 })))
      : null,
    h("ul", { class: "jmap", onclick: (e) => goToCard(e, ctx.idp) }, rows),
    dropped ? h("p", { class: "jmap-more" }, `and ${plural(dropped, "more place")} with fewer or lower findings`) : null,
    h("p", { class: "jmap-key" },
      ["on", "off", "na"].filter((k) => seen.has(k)).map((k) => h("span", {}, h("i", { class: `sq sq-${k}`, "aria-hidden": "true" }), REACH[k][1])),
      h("span", {}, "H high · N notable · I info"),
      h("a", { href: "docs/report/#the-trail" }, "How to read a trail")),
  ], ctx.nested);
}

function facts(rows) {
  return h("dl", { class: "facts" }, rows.filter(([, v]) => v != null && v !== "").map(([k, v]) => h("div", {}, h("dt", {}, k), h("dd", {}, v))));
}

function plainList(items) {
  return h("ul", { class: "plain" }, items.map((i) => h("li", {}, code(i))));
}

function jarDetails(r, id, nested) {
  return fold("About this jar", () => {
    const o = r.obfuscation;
    const reach = r.reachability;
    const parts = [
      facts([
        ["File", r.file.name],
        ["Size", size(r.file.size)],
        ["SHA-256", h("code", {}, r.file.sha256)],
        ["Classes", String(r.stats.classes)],
        ["Entries", String(r.stats.entries)],
        ["Obfuscation", `${o.score}/100 (short or garbled names ${Math.round(o.short_names * 100)}%, default package ${Math.round(o.default_package * 100)}%, encoded strings ${Math.round(o.encoded_strings * 100)}%)`],
        ["Call graph", reach ? `${reach.reached_methods} of ${reach.methods} methods reached from ${plural(reach.entry_points.length, "entry point")}` : null],
        ["Entry points", reach ? breakable(reach.entry_points.slice(0, 3).join(", ") + (reach.entry_points.length > 3 ? ` and ${reach.entry_points.length - 3} more` : "")) : null],
      ]),
    ];
    for (const p of r.plugins) {
      parts.push(h(hx(4, nested), {}, `${p.descriptor} (${p.platform})`));
      parts.push(facts([
        ["Name", p.name],
        ["Version", p.version],
        ["Authors", p.authors.length ? p.authors.join(", ") : null],
        ["Website", p.website],
        ["Main class", p.main ? breakable(p.main) : null],
        ["Bootstrapper", p.bootstrapper ? breakable(p.bootstrapper) : null],
        ["Loader", p.loader ? breakable(p.loader) : null],
        ["Folia", p.folia_supported ? "Declares Folia support" : null],
        ["Commands", p.commands.length ? p.commands.map((c) => `/${c}`).join(", ") : null],
        ["Permissions", p.permissions.length ? breakable(p.permissions.join(", ")) : null],
        ["Dependencies", p.dependencies.length ? p.dependencies.join(", ") : null],
        ["Optional dependencies", p.optional_dependencies.length ? p.optional_dependencies.join(", ") : null],
      ]));
      if (p.maven_libraries.length) {
        parts.push(h(hx(4, nested), {}, "Libraries the server downloads at startup"), plainList(p.maven_libraries));
      }
    }
    if (r.libraries.length) {
      parts.push(h(hx(4, nested), {}, "Bundled libraries"), plainList(r.libraries.map((l) => `${l.group}:${l.artifact}:${l.version}`)));
    }
    if (r.nested.length) {
      parts.push(h(hx(4, nested), {}, "Nested jars"), plainList(r.nested.map((n) => `${n.path} (${size(n.size)})`)));
    }
    return parts;
  }, { id, heading: hx(3, nested) });
}

function warningsSection(warnings, id, nested) {
  if (!warnings.length) return null;
  return fold(`Warnings (${warnings.length})`, () =>
    h("ul", { class: "plain" }, warnings.map((w) => h("li", {}, h("code", {}, w.code), " ", w.location ? [code(w.location), ": "] : null, w.message))), { id, heading: hx(3, nested) });
}

const pluginName = (r) => {
  const p = r.plugins[0];
  return p?.name ? `${p.name} ${p.version ?? ""}`.trim() : r.file.name;
};

let reports = 0;

function renderReport(r, { nested = false, ms, sample = false, file = null } = {}) {
  const p = r.plugins[0];
  const ctx = { nested, jar: r.file.name, idp: `r${++reports}-` };
  const bad = unreadable(r.warnings);
  // An unreadable file gets no class count, score or counts: nothing in it was analyzed.
  const meta = [
    p?.name ? r.file.name : null,
    p ? `${p.platform} plugin` : null,
    size(r.file.size),
    `SHA-256 ${r.file.sha256.slice(0, 12)}`,
    bad ? null : plural(r.stats.classes, "class", "classes"),
    bad ? null : `obfuscation ${r.obfuscation.score}/100`,
  ].filter(Boolean);
  const id = (x) => (nested ? null : x);
  return frame({
    nested,
    label: nested ? null : sample ? "scan · sample" : "scan",
    bad,
    title: pluginName(r),
    meta,
    counts: bad ? null : countList(counts(r.findings)),
    download: downloadButton(`${r.file.name}.jarsnitch.json`, r),
    toc: [
      bad || nested || sample || !file ? null : ["r-official", "Official release"],
      bad || !r.findings.some((f) => f.evidence.some((e) => e.location)) ? null : ["r-map", "Where in the jar"],
      bad ? null : ["r-findings", "Findings", r.findings.length],
      ["r-about", "About this jar"],
      r.warnings.length ? ["r-warnings", "Warnings", r.warnings.length] : null,
    ],
    prov: nested ? null : prov(r.tool_version, ms, sample),
  }, [
    bad ? h("p", { class: "banner error" }, "This file isn't a readable jar or zip, so nothing in it was checked.") : null,
    r.partial ? h("p", { class: "banner warn" }, "Partial report: a scan limit was hit, so part of the jar wasn't checked. A partial report is not a clean one.") : null,
    bad || nested || sample || !file ? null : officialSection([{ file, report: r }], "r-official"),
    bad ? null : jarMap(r, id("r-map"), ctx),
    bad ? null : findingsSection(r.findings, r.reachability, id("r-findings"), ctx),
    jarDetails(r, id("r-about"), nested),
    warningsSection(r.warnings, id("r-warnings"), nested),
  ]);
}

/** Puts a note at the top of a report's main column. */
function withNote(report, note) {
  if (note) report.querySelector(".report-main").prepend(h("p", { class: "banner note" }, note));
  return report;
}

/** Severity counts for a summary line, leaving out zeros. */
function badges(c) {
  const shown = SEVS.filter((s) => c[s]);
  if (!shown.length) return h("span", { class: "muted" }, "No findings");
  return h("span", { class: "badges" }, shown.map((s) => h("span", { class: `sev sev-${s}` }, `${c[s]} ${LABEL[s]}`)));
}

/** Several jars: one row each, most High then most Notable findings first, full report on open. */
function renderMany(scanned, ms) {
  const rank = ({ report: r }) => (r && !unreadable(r.warnings) ? counts(r.findings) : { high: -1, notable: -1 });
  const order = [...scanned].sort((a, b) => {
    const [x, y] = [rank(a), rank(b)];
    return y.high - x.high || y.notable - x.notable || a.file.name.localeCompare(b.file.name, undefined, { numeric: true });
  });
  const reports = scanned.filter((s) => s.report).map((s) => s.report);
  return frame({
    label: "scan · several jars",
    title: plural(scanned.length, "jar"),
    meta: "Most high, then most notable findings first. Open a plugin for its full report.",
    counts: countList(counts(reports.flatMap((r) => r.findings))),
    download: downloadButton("plugins.jarsnitch.json", reports),
    prov: prov(reports[0]?.tool_version, ms),
    toc: [["r-official", "Official releases"], ["r-jars", "Plugins", scanned.length]],
  }, [
    officialSection(scanned.filter((s) => s.report && !unreadable(s.report.warnings)), "r-official"),
    section("jars", "r-jars", "Plugins", plural(scanned.length, "jar"), h("div", { class: "list" }, order.map(jarRow))),
  ]);
}

function jarRow({ file, report: r, error }) {
  if (!r) return h("div", { class: "row jar-error" }, h("span", { class: "text" }, code(file.name)), h("span", { class: "muted" }, error));
  const p = r.plugins[0];
  const summary = h("span", { class: "jar-sum" },
    h("span", { class: "jar-name" }, p?.name ? [`${p.name} ${p.version ?? ""}`.trim(), h("span", { class: "jar-file" }, breakable(file.name))] : breakable(file.name)),
    unreadable(r.warnings) ? h("span", { class: "muted" }, "Not a readable jar or zip") : badges(counts(r.findings)));
  const c = counts(r.findings);
  const top = unreadable(r.warnings) ? "none" : SEVS.find((s) => c[s]) ?? "none";
  return fold(summary, () => renderReport(r, { nested: true }), { cls: `fold jar sev-${top}` });
}

// Diff.

const KIND = {
  command: (v) => ["command ", h("code", {}, `/${v}`)],
  permission: (v) => ["permission ", code(v)],
  "maven-library": (v) => ["library downloaded at startup ", code(v)],
  library: (v) => ["bundled library ", code(v)],
  dependency: (v) => ["dependency ", code(v)],
  "optional-dependency": (v) => ["optional dependency ", code(v)],
};

function itemRow(item, verb) {
  if (item.kind === "class") {
    return h("div", { class: `row sev-${item.severity}` },
      sevBadge(item.severity), h("span", { class: "text" }, `${verb} class `, code(item.value)),
      ruleChip(item.rules));
  }
  return h("div", { class: "row sev-info" }, sevBadge("info"), h("span", { class: "text" }, `${verb} `, KIND[item.kind](item.value)));
}

/** An item on one row: a finding by its title, anything else as itemRow has it. */
function compactRow(item, verb) {
  if (item.kind !== "finding") return itemRow(item, verb);
  const f = item.finding;
  return h("div", { class: `row sev-${f.severity}` },
    sevBadge(f.severity), h("span", { class: "text" }, f.title, f.value ? [": ", code(f.value)] : null), ruleChip(f.rule));
}

function changeRow(c) {
  const show = (v) => (v == null ? "(none)" : c.kind === "nested" ? v.slice(0, 12) : v);
  const what = {
    library: `Bundled library ${c.key}`,
    "maven-library": `Library downloaded at startup ${c.key}`,
    nested: `Nested jar ${c.key} (SHA-256)`,
  }[c.kind] ?? c.key;
  return h("div", { class: "row change" },
    h("span", { class: "k" }, breakable(what), h("span", { class: "vh" }, ": ")),
    h("span", { class: "v" }, code(show(c.old), "old"), h("span", { class: "to" }, " → "), code(show(c.new))));
}

/** Added items in diff order, with runs of findings grouped like in a report. */
function addedList(items, jar) {
  const out = [];
  let run = [];
  let n = 0;
  const flush = () => {
    out.push(findingCards(cardGroups(run), { nested: false, jar, idp: `d${reports}-${n++}-` }));
    run = [];
  };
  for (const i of items) {
    if (i.kind === "finding") run.push(i.finding);
    else {
      flush();
      out.push(itemRow(i, "New"));
    }
  }
  flush();
  return out;
}

function renderDiff(d, { ms, sample = false, nested = false, adds = "This update adds", removes = "This update removes", tag = null } = {}) {
  reports++;
  const id = (x) => (nested ? null : x);
  const label = (side) => {
    const p = side.plugins[0];
    return p?.name ? `${p.name} ${p.version ?? ""}`.trim() : side.file.name;
  };
  const [a, b] = [label(d.old), label(d.new)];
  const sameName = d.old.plugins[0]?.name && d.old.plugins[0].name === d.new.plugins[0]?.name;
  const to = h("span", { class: "to" }, " → ");
  const title = sameName ? [a, to, d.new.plugins[0].version ?? "?"] : [a, to, b];
  const addedFindings = d.added.filter((i) => i.kind === "finding").map((i) => i.finding);

  const unchanged = [];
  for (const k of d.unchanged) {
    const last = unchanged.at(-1);
    if (last?.rule === k.rule) last.n++;
    else unchanged.push({ rule: k.rule, n: 1 });
  }
  const items = (n, one = "item") => (n ? plural(n, one) : null);

  return frame({
    nested,
    label: tag ?? (sample ? "compare · sample" : "compare"),
    title,
    meta: [[h("span", { class: "nw" }, d.old.file.name), " → ", h("span", { class: "nw" }, d.new.file.name)], `obfuscation ${d.obfuscation.old} → ${d.obfuscation.new}`],
    counts: countList(counts(addedFindings), " added"),
    download: downloadButton(`${d.old.file.name}-vs-${d.new.file.name}.jarsnitch.json`, d),
    toc: [
      ["r-adds", adds, d.added.length],
      ["r-removes", removes, d.removed.length],
      d.changed.length ? ["r-changes", "Changes", d.changed.length] : null,
      renamed(d) ? null : ["r-code", "Code"],
      unchanged.length ? ["r-unchanged", "Unchanged", unchanged.length] : null,
    ],
    prov: nested ? null : prov(d.tool_version, ms, sample),
  }, [
    d.warnings.map((w) => h("p", { class: "banner warn" }, w.message)),
    section("adds", id("r-adds"), adds, items(d.added.length),
      d.added.length
        ? h("div", { class: "list" }, addedList(d.added, d.new.file.name))
        : h("p", { class: "empty" }, "Nothing new from the rule list."), nested),
    section("removes", id("r-removes"), removes, items(d.removed.length),
      d.removed.length ? h("div", { class: "list" }, d.removed.map((i) => compactRow(i, "Removed"))) : h("p", { class: "empty" }, "Nothing."), nested),
    d.changed.length
      ? section("changes", id("r-changes"), "Changes", items(d.changed.length, "field"), [
          h("div", { class: "list" }, d.changed.map(changeRow)),
          d.changed.some((c) => c.kind === "plugin-field" && / (authors|website)$/.test(c.key))
            ? h("p", { class: "after" }, "A different author or website can mean the plugin has a new owner.")
            : null,
        ], nested)
      : null,
    codeSection(d, id("r-code"), nested),
    unchanged.length
      ? section("unchanged", id("r-unchanged"), "Unchanged", items(unchanged.length, "rule"),
          h("ul", { class: "chips" }, unchanged.map((u) => h("li", {}, ruleChip(u.rule), u.n > 1 ? ` ×${u.n}` : null))), nested)
      : null,
  ]);
}

const renamed = (d) => d.warnings.some((w) => w.code === "diff.classes-renamed");
const CLASS_LIST = 10;
const MEMBER_LIST = 12;

/** "onEnable()V" → "onEnable()", "load(Ljava/io/File;)…" → "load(File)". */
function methodName(m) {
  const at = m.indexOf("(");
  return h("code", {}, at < 0 ? m : [m.slice(0, at), `(${params(m.slice(at))})`]);
}

/** "STATS:Ljava/lang/String;" → "STATS". */
const fieldName = (f) => h("code", {}, f.split(":")[0]);

function classChangeRow(x) {
  const lines = [
    x.duplicate ? h("li", {}, "More than one entry has this name, with different code; the server may load any of them, so they're compared together.") : null,
    x.header_changed ? h("li", {}, "Superclass, interfaces or access flags changed") : null,
    ...[["Added method", x.methods_added, methodName], ["Removed method", x.methods_removed, methodName], ["Changed method", x.methods_changed, methodName], ["Added field", x.fields_added, fieldName], ["Removed field", x.fields_removed, fieldName], ["Changed field", x.fields_changed ?? [], fieldName]]
      .flatMap(([k, list, show]) => list.map((m) => h("li", {}, `${k} `, show(m)))),
  ].filter(Boolean);
  const calls = x.calls_added;
  return h("div", { class: `row class-change sev-${x.severity}` },
    sevBadge(x.severity),
    h("span", { class: "text" }, "Changed class ", code(x.class)),
    x.rules_added.length ? ruleChip(x.rules_added) : h("span", {}),
    h("ul", { class: "members" },
      lines.slice(0, MEMBER_LIST),
      lines.length > MEMBER_LIST ? h("li", { class: "muted" }, `and ${lines.length - MEMBER_LIST} more`) : null,
      calls.length ? h("li", {}, "Now calls ", calls.slice(0, 8).map((c, i) => [i ? ", " : null, code(c)]), calls.length > 8 ? ` and ${calls.length - 8} more` : null) : null));
}

/** The class-by-class comparison. Changed classes with notable or High code come first. */
function codeSection(d, id, nested) {
  if (renamed(d)) return null;
  const c = d.code;
  const same = sameCode(d);
  const notable = c.changed_classes.filter((x) => x.severity !== "info").length;
  const shown = c.changed_classes.slice(0, Math.max(notable, CLASS_LIST));
  const rest = c.changed_classes.slice(shown.length);
  const files = [...c.added_files.map((f) => ["Added", f]), ...c.removed_files.map((f) => ["Removed", f]), ...c.changed_files.map((f) => ["Changed", f])];
  const classList = (list) => h("ul", { class: "plain" }, list.map((x) => h("li", {}, code(x))));
  return section("code", id, "Code", same ? null : plural(c.changed_classes.length, "changed class", "changed classes"), [
    h("p", { class: "muted" },
      same
        ? `The same code in both: ${plural(c.unchanged_classes, "class", "classes")}, compared method by method.`
        : `${plural(c.changed_classes.length, "class", "classes")} changed, ${c.added_classes.length} added, ${c.removed_classes.length} removed, ${c.unchanged_classes} unchanged. Classes are compared method by method, so a rebuild that only renumbers constants isn't a change.`),
    shown.length ? h("div", { class: "list" }, shown.map(classChangeRow)) : null,
    rest.length ? fold(`${plural(rest.length, "more changed class", "more changed classes")}`, () => h("div", { class: "list" }, rest.map(classChangeRow)), { cls: "fold info" }) : null,
    c.added_classes.length ? fold(`New classes (${c.added_classes.length})`, () => classList(c.added_classes), { cls: "fold info" }) : null,
    c.removed_classes.length ? fold(`Removed classes (${c.removed_classes.length})`, () => classList(c.removed_classes), { cls: "fold info" }) : null,
    files.length ? fold(`Other files (${files.length})`, () => h("ul", { class: "plain" }, files.map(([k, f]) => h("li", {}, `${k} `, code(f)))), { cls: "fold info" }) : null,
  ], nested);
}

// Official releases.

const SITE = { modrinth: "Modrinth", hangar: "Hangar" };
const release = (s) => `${s.project}${s.version ? ` ${s.version}` : ""} on ${SITE[s.site]}`;
const outLink = (href, text) => h("a", { href, target: "_blank", rel: "noopener noreferrer" }, text);
const STATUS = {
  official: ["ok", "Official file"],
  "same-code": ["ok", "Same code"],
  differs: ["high", "Code differs"],
  "not-found": ["none", "Not found"],
  unreadable: ["none", "Unreadable"],
};

/**
 * Opt-in: checks the jars against the files their projects published, and for updates.
 * Nothing leaves the tab until the button is pressed.
 */
function officialSection(entries, id) {
  if (!entries.length) return null;
  // The scan or comparison this section belongs to. A newer one makes it stale: its check
  // doesn't start, and one already running is aborted and leaves the new report alone.
  const born = generation;
  const many = entries.length > 1;
  const button = h("button", { type: "button", class: "btn" }, many ? "Check all against official releases" : "Check against official releases");
  const note = h("p", { class: "muted", role: "status" });
  const body = h("div", { class: "official" },
    h("p", {}, many
      ? "See whether each jar is the file its project published on Modrinth or Hangar, what's different if it isn't, and whether there's an update and what it adds."
      : "See whether this is the file its project published on Modrinth or Hangar, what's different if it isn't, and whether there's an update and what it adds."),
    h("p", { class: "muted disclose" }, `This sends ${many ? "each jar's" : "the jar's"} SHA-512 and SHA-256 hashes and plugin name and version to api.modrinth.com and hangar.papermc.io, and downloads official files from cdn.modrinth.com to compare with. The ${many ? "jars themselves aren't" : "jar itself isn't"} sent.`),
    h("div", { class: "head-row" }, button), note);
  const stale = () => born !== generation;
  /** Back to the state before the press, so a stopped check doesn't leave "Looking up…". */
  const reset = () => {
    note.textContent = "";
    button.disabled = false;
  };
  button.addEventListener("click", async () => {
    if (stale()) {
      button.disabled = true;
      note.textContent = "This report is older than the last scan or comparison. Scan the jar again to check it.";
      return;
    }
    const { signal } = checks;
    button.disabled = true;
    note.textContent = "Checking…";
    try {
      const checked = await checkAll(entries, { diff: diffFiles, progress: (t) => stale() || (note.textContent = t), signal });
      if (stale()) return reset();
      body.replaceChildren(...[
        many ? h("p", { class: "muted" }, checkSummary(checked)) : null,
        h("div", { class: "list" }, checked.map((c) => checkRow(c, many))),
        h("p", { class: "after" }, "Official means the project published that file there. It isn't a check of what the code does: that's what the findings are for."),
      ].filter(Boolean));
      const [c] = checked;
      const adds = c.status !== "differs" && addsHigh(c.official?.diff) ? ", but it adds a High finding to the official file" : "";
      const update = c.update && addsHigh(c.update.diff) ? `; its update ${c.update.version} adds a High finding` : "";
      setStatus(many ? checkSummary(checked) : `Checked ${entries[0].file.name}: ${STATUS[c.status][1].toLowerCase()}${adds}${update}.`);
    } catch (e) {
      if (stale()) return reset();
      note.textContent = e.message;
      button.disabled = false;
    }
  });
  return section("official", id, many ? "Official releases" : "Official release", null, body);
}

function checkSummary(checked) {
  const n = (s) => checked.filter((c) => c.status === s).length;
  const high = (list) => (list.length ? ` (${list.length} ${list.length === 1 ? "adds" : "add"} a High finding)` : "");
  const updates = checked.filter((c) => c.update);
  const sameHigh = checked.filter((c) => c.status !== "differs" && addsHigh(c.official?.diff));
  return `${plural(checked.length, "jar")}: ${n("official")} official, ${n("same-code")} same code${high(sameHigh)}, ${n("differs")} code differs, ${n("not-found")} not found · ${plural(updates.length, "update")} available${high(updates.filter((c) => addsHigh(c.update.diff)))}.`;
}

/** A file the server or Java reads to decide what runs: a plugin descriptor, the manifest or a service list. */
const RUNTIME = ["plugin.yml", "paper-plugin.yml", "bungee.yml", "velocity-plugin.json", "fabric.mod.json", "quilt.mod.json", "meta-inf/mods.toml", "meta-inf/neoforge.mods.toml", "meta-inf/manifest.mf", "meta-inf/coremods.json"];
const runtimeFile = (path) => {
  const p = path.toLowerCase();
  // Mixin configs pick the classes that rewrite other code as it loads.
  const mixins = p.endsWith(".json") && (p.endsWith(".mixins.json") || p.split("/").pop().startsWith("mixins."));
  return RUNTIME.includes(p) || path.startsWith("META-INF/services/") || mixins;
};

/** The first `max` names as code, then how many more. */
const codeList = (items, max) => [items.slice(0, max).map((f, i) => [i ? ", " : null, code(f)]), items.length > max ? `, and ${items.length - max} more` : null];

const itemSeverity = (i) => (i.kind === "finding" ? i.finding.severity : i.severity ?? "info");

const compareFold = (summary, d, open = false) =>
  fold(summary, () => renderDiff(d, { nested: true, adds: "This jar adds", removes: "This jar removes" }), { cls: "fold info", open });

function checkRow(c, many) {
  const o = c.official;
  const u = c.update;
  // A file of this version in the plugin's own project that isn't the same plugin.
  const unlike = c.status === "not-found" && c.plugin && c.version ? c.unlike : null;
  // The official file's code, or its bytes, but this jar adds a High finding to it.
  const high = c.status !== "differs" && addsHigh(o?.diff);
  const [tone, word] = STATUS[c.status];
  const sites = c.hangar ? "Modrinth or Hangar" : "Modrinth";
  const line = {
    official: () => ["The same bytes as ", outLink(o.url, release(o)), "."],
    "same-code": () => ["The same code as ", outLink(o.url, release(o)), ", but not the same file", sameCodeFiles(o.diff), "."],
    differs: () => ["Its code differs from ", outLink(o.url, release(o)), "."],
    // Only what the page asked: Hangar by bytes (server plugins only), versions on Modrinth only.
    "not-found": () => unlike
      ? [`No file with these bytes on ${sites}. `, outLink(unlike.url, release(unlike)),
          ` has a ${c.plugin} ${c.version} that isn't this plugin: its name, or its main class and most of its classes, differ from this jar's.`]
      : [
          `No file with these bytes on ${sites}${c.plugin && c.version ? `, and no official ${c.plugin} ${c.version} on Modrinth to compare with` : ""}.`,
          c.hangar && c.plugin && c.version ? [" The command line's ", code("jarsnitch check"), " also compares with Hangar releases."] : null,
          " Paid, private and custom builds can't be checked this way.",
        ],
    unreadable: () => ["Not a readable jar."],
  }[c.status]();
  const parts = [h("p", { class: "chk-line" }, h("span", { class: `chk chk-${high ? "high" : tone}` }, word), h("span", {}, line))];
  // Anyone can publish a copy as their own project.
  if (o?.named === false) {
    parts.push(h("p", { class: "muted" }, `This project isn't named like the plugin (${c.plugin}). Make sure it's the project you meant.`));
  }
  if (c.status === "same-code" && o.diff) {
    const k = o.diff.code;
    const read = [...k.changed_files, ...k.added_files, ...k.removed_files].filter(runtimeFile);
    if (read.length) parts.push(h("p", { class: "muted" }, "The server or Java reads ", codeList(read, 4), ": it sets commands, permissions or what loads, so compare it with the official file."));
  }
  // What a differing jar adds is in its open comparison; a same-code one lists it here, like the CLI.
  const adds = o?.diff && c.status !== "differs" ? o.diff.added.filter((i) => itemSeverity(i) !== "info") : [];
  if (adds.length) {
    parts.push(h("div", { class: "adds" }, h("p", { class: "muted" }, "This jar adds:"), h("div", { class: "list" }, adds.map((i) => compactRow(i, "New")))));
  }
  if (o?.diff && c.status !== "official") {
    parts.push(compareFold(c.status === "differs" ? "What this jar adds to the official file" : "Compare with the official file", o.diff, (c.status === "differs" || high) && !many));
  }
  if (unlike?.diff) parts.push(compareFold("Compare with that file", unlike.diff));
  if (u) {
    const date = u.published ? `, ${u.published.slice(0, 10)}` : "";
    // Findings, like the summary and the exit code; new classes with findings the jar already has are said after.
    const added = u.diff ? counts(u.diff.added.filter((i) => i.kind === "finding").map((i) => i.finding)) : null;
    const classes = u.diff ? u.diff.added.filter((i) => i.kind !== "finding" && itemSeverity(i) !== "info").length : 0;
    const says = u.diff && (u.diff.old.partial || u.diff.new.partial)
      ? " Comparison is incomplete; findings cover only the scanned contents."
      : !added
      ? ""
      : added.high + added.notable
        ? ` It adds ${added.high} high and ${added.notable} notable findings.`
        : classes
          ? " It adds no high or notable findings, but new classes with them."
          : " It adds no high or notable findings.";
    parts.push(h("p", { class: "chk-line" }, h("span", { class: `chk chk-${added?.high ? "high" : "up"}` }, "Update"), h("span", {}, outLink(u.url, `${u.version}${date}`), ` on ${SITE[u.site]}.${says}`)));
    parts.push(u.diff
      ? fold("What the update changes", () => renderDiff(u.diff, { nested: true }), { cls: "fold info" })
      : h("p", { class: "muted" }, "Not compared: that release has no file for this server software."));
  }
  for (const e of c.errors) parts.push(h("p", { class: "muted" }, `Couldn't check everything: ${e}`));
  return h("div", { class: `check sev-${alarming(c) ? "high" : "info"}` },
    many ? h("h4", {}, c.plugin ? `${c.plugin} ${c.version ?? ""}`.trim() : c.file, h("span", { class: "jar-file" }, breakable(c.file))) : null,
    parts);
}

function sameCodeFiles(d) {
  const c = d?.code;
  if (!c) return null;
  const files = [...c.changed_files, ...c.added_files, ...c.removed_files];
  if (!files.length) return ": every file inside is the same, only the zip differs";
  return [": other files differ (", codeList(files, 4), ")"];
}

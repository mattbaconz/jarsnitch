// Page logic: file picking, talking to the worker, rendering reports.
// Report text comes from the scanned jar, so it is only ever inserted as text.

const status = document.getElementById("status");
const results = document.getElementById("results");
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

function setStatus(text, error = false) {
  status.textContent = text;
  status.classList.toggle("error", error);
}

// Worker. Started right away so the wasm is fetched during page load and never later.

let fatal = null;
let nextId = 0;
let latest = 0;
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
  latest = id;
  const loaded = await Promise.all(files.map(async (f) => ({ name: f.name, bytes: await f.arrayBuffer() })));
  const answer = new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
  worker.postMessage({ id, op, files: loaded }, loaded.map((f) => f.bytes));
  return { id, ...(await answer) };
}

// Tabs.

const tabs = [...document.querySelectorAll('[role="tab"]')];

function selectTab(tab) {
  for (const t of tabs) {
    const on = t === tab;
    t.setAttribute("aria-selected", String(on));
    t.tabIndex = on ? 0 : -1;
    document.getElementById(t.getAttribute("aria-controls")).hidden = !on;
  }
}

for (const tab of tabs) {
  tab.addEventListener("click", () => selectTab(tab));
  tab.addEventListener("keydown", (e) => {
    const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
    if (!step) return;
    const next = tabs[(tabs.indexOf(tab) + step + tabs.length) % tabs.length];
    selectTab(next);
    next.focus();
  });
}

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
    if (input.files[0]) pick(slot, input.files[0]);
    input.value = "";
  });
}

document.getElementById("swap").addEventListener("click", () => {
  [pair.old, pair.new] = [pair.new, pair.old];
  showFile("old", pair.old);
  showFile("new", pair.new);
  if (pair.old && pair.new) compare();
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
  const files = [...e.dataTransfer.files];
  const zone = e.target.closest?.(".drop");
  if (files.length === 1 && zone) {
    pick(zone.dataset.slot, files[0]);
  } else if (files.length === 1) {
    selectTab(tabs[0]);
    pick("scan", files[0]);
  } else if (files.length === 2) {
    // Older first when the names carry versions: Foo-1.9.jar before Foo-1.10.jar.
    files.sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true }));
    selectTab(tabs[1]);
    [pair.old, pair.new] = files;
    showFile("old", pair.old);
    showFile("new", pair.new);
    compare();
  } else {
    setStatus("Drop one jar to scan it, or two to compare versions.", true);
  }
});

async function scan(file) {
  setStatus(`Analyzing ${file.name}…`);
  try {
    const { id, result, ms } = await analyze("scan", [file]);
    if (id !== latest) return;
    const c = counts(result.findings);
    if (unreadable(result.warnings)) setStatus(`${file.name} isn't a readable jar or zip.`, true);
    else setStatus(`Scanned ${file.name} in ${seconds(ms)}: ${c.high} high, ${c.notable} notable, ${c.info} info.`);
    show(renderReport(result));
  } catch (e) {
    setStatus(e.message, true);
  }
}

async function compare() {
  const { old: a, new: b } = pair;
  setStatus(`Comparing ${a.name} with ${b.name}…`);
  try {
    const { id, result, ms } = await analyze("diff", [a, b]);
    if (id !== latest) return;
    const bad = [[a, result.old], [b, result.new]].find(([, side]) => unreadable(side.warnings));
    if (bad) {
      results.replaceChildren();
      return setStatus(`${bad[0].name} isn't a readable jar or zip, so there's nothing to compare.`, true);
    }
    const findings = result.added.filter((i) => i.kind === "finding").map((i) => i.finding);
    const c = counts(findings);
    setStatus(`Compared in ${seconds(ms)}. The update adds ${c.high} high, ${c.notable} notable and ${c.info} info findings.`);
    show(renderDiff(result));
  } catch (e) {
    setStatus(e.message, true);
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
const size = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1e3))} KB`);
const seconds = (ms) => (ms < 1000 ? `${Math.max(1, Math.round(ms))} ms` : `${(ms / 1000).toFixed(1)} s`);
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
const where = (e) => {
  const loc = e.location || "(jar)";
  return e.method ? `${loc}.${e.method.split("(")[0]}` : loc;
};
const sevBadge = (severity) => h("span", { class: `sev sev-${severity}` }, LABEL[severity]);
const unreadable = (warnings) => warnings.some((w) => w.code === "zip.unreadable" && !w.location);

function countList(c, suffix = "") {
  return h("ul", { class: "counts", "aria-label": "Findings by severity" },
    ["high", "notable", "info"].map((s) => h("li", {}, h("span", { class: `sev sev-${s}` }, `${c[s]} ${LABEL[s]}${suffix}`))));
}

function downloadButton(fileName, data) {
  return h("button", {
    type: "button",
    class: "button",
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

function findingCard(f) {
  const title = f.value ? [f.title, ": ", h("code", {}, f.value)] : f.title;
  return h("article", { class: `finding sev-${f.severity}` },
    h("div", { class: "finding-head" }, sevBadge(f.severity), h("h4", {}, title), h("code", { class: "rule" }, f.rule)),
    h("p", { class: "why" }, f.why),
    h("p", { class: "legit" }, "Common legitimate uses: ", f.legit_uses),
    evidence(f));
}

/** Cards for findings in report order; findings that share a rule (hosts, say) share a card. */
function findingCards(findings) {
  const groups = [];
  for (const f of findings) {
    const last = groups.at(-1);
    if (last?.[0].rule === f.rule) last.push(f);
    else groups.push([f]);
  }
  return groups.map((g) => (g.length === 1 ? findingCard(g[0]) : groupCard(g)));
}

function groupCard(group) {
  const f = group[0];
  const row = (x) => h("li", {}, h("code", {}, x.value ?? "(no value)"), x.evidence[0] ? h("span", { class: "detail" }, "in ", where(x.evidence[0]), x.evidence_total > 1 ? ` and ${plural(x.evidence_total - 1, "more place")}` : "") : null);
  const shown = 8;
  return h("article", { class: `finding sev-${f.severity}` },
    h("div", { class: "finding-head" }, sevBadge(f.severity), h("h4", {}, `${f.title} (${group.length})`), h("code", { class: "rule" }, f.rule)),
    h("p", { class: "why" }, f.why),
    h("p", { class: "legit" }, "Common legitimate uses: ", f.legit_uses),
    h("ul", { class: "values" }, group.slice(0, shown).map(row)),
    group.length > shown
      ? h("details", { class: "where" }, h("summary", {}, `Show ${group.length - shown} more`), h("ul", { class: "values" }, group.slice(shown).map(row)))
      : null);
}

function evidence(f) {
  const [first] = f.evidence;
  if (!first) return null;
  const line = (e) => [h("code", {}, where(e)), e.detail ? h("span", { class: "detail" }, e.detail) : null];
  if (f.evidence_total === 1) return h("p", { class: "where" }, "Found in ", line(first));
  const hidden = f.evidence_total - f.evidence.length;
  return h("details", { class: "where" },
    h("summary", {}, "Found in ", h("code", {}, where(first)), ` and ${plural(f.evidence_total - 1, "more place")}`),
    h("ul", {}, f.evidence.map((e) => h("li", {}, line(e)))),
    hidden > 0 ? h("p", { class: "muted" }, `${hidden} more not listed.`) : null);
}

/** A collapsed section whose content is only built when it's opened. */
function fold(summary, build, open = false) {
  const d = h("details", { class: "fold", open }, h("summary", {}, summary));
  const fill = () => {
    if (d.dataset.filled) return;
    d.dataset.filled = "1";
    d.append(build());
  };
  if (open) fill();
  else d.addEventListener("toggle", fill);
  return d;
}

function findingsSection(findings) {
  const important = findings.filter((f) => f.severity !== "info");
  const info = findings.filter((f) => f.severity === "info");
  return h("section", { class: "section", "aria-label": "Findings" },
    findings.length === 0
      ? h("p", {}, "jarsnitch found nothing from its rule list in this jar.")
      : [
          important.length ? h("div", { class: "list" }, findingCards(important)) : h("p", { class: "muted" }, "No high or notable findings."),
          info.length ? fold(`Info findings (${info.length})`, () => h("div", { class: "list" }, findingCards(info))) : null,
        ]);
}

function facts(rows) {
  return h("dl", { class: "facts" }, rows.filter(([, v]) => v != null && v !== "").map(([k, v]) => [h("dt", {}, k), h("dd", {}, v)]));
}

function plainList(items) {
  return h("ul", { class: "plain" }, items.map((i) => h("li", {}, h("code", {}, i))));
}

function jarDetails(r) {
  return fold("About this jar", () => {
    const o = r.obfuscation;
    const parts = [
      facts([
        ["File", r.file.name],
        ["Size", size(r.file.size)],
        ["SHA-256", h("code", {}, r.file.sha256)],
        ["Classes", String(r.stats.classes)],
        ["Entries", String(r.stats.entries)],
        ["Obfuscation", `${o.score}/100 (short or garbled names ${Math.round(o.short_names * 100)}%, default package ${Math.round(o.default_package * 100)}%, encoded strings ${Math.round(o.encoded_strings * 100)}%)`],
      ]),
    ];
    for (const p of r.plugins) {
      parts.push(h("h4", {}, `${p.descriptor} (${p.platform})`));
      parts.push(facts([
        ["Name", p.name],
        ["Version", p.version],
        ["Main class", p.main],
        ["Bootstrapper", p.bootstrapper],
        ["Loader", p.loader],
        ["Folia", p.folia_supported ? "Declares Folia support" : null],
        ["Commands", p.commands.length ? p.commands.map((c) => `/${c}`).join(", ") : null],
        ["Permissions", p.permissions.length ? p.permissions.join(", ") : null],
      ]));
      if (p.maven_libraries.length) {
        parts.push(h("h4", {}, "Libraries the server downloads at startup"), plainList(p.maven_libraries));
      }
    }
    if (r.libraries.length) {
      parts.push(h("h4", {}, "Bundled libraries"), plainList(r.libraries.map((l) => `${l.group}:${l.artifact}:${l.version}`)));
    }
    if (r.nested.length) {
      parts.push(h("h4", {}, "Nested jars"), plainList(r.nested.map((n) => `${n.path} (${size(n.size)})`)));
    }
    return parts;
  });
}

function warningsSection(warnings) {
  if (!warnings.length) return null;
  return fold(`Warnings (${warnings.length})`, () =>
    h("ul", { class: "plain" }, warnings.map((w) => h("li", {}, h("code", {}, w.code), " ", w.location ? [h("code", {}, w.location), ": "] : null, w.message))));
}

function renderReport(r) {
  const p = r.plugins[0];
  const name = p?.name ? `${p.name} ${p.version ?? ""}`.trim() : r.file.name;
  const meta = [
    p?.name ? r.file.name : null,
    p ? `${p.platform} plugin` : null,
    size(r.file.size),
    `SHA-256 ${r.file.sha256.slice(0, 12)}`,
    plural(r.stats.classes, "class", "classes"),
    `obfuscation ${r.obfuscation.score}/100`,
  ].filter(Boolean).join(" · ");
  return h("article", { class: "report" },
    h("header", { class: "report-head" },
      h("h2", { tabindex: "-1" }, name),
      h("p", { class: "meta" }, meta),
      h("div", { class: "head-row" }, countList(counts(r.findings)), downloadButton(`${r.file.name}.jarsnitch.json`, r))),
    unreadable(r.warnings) ? h("p", { class: "banner error" }, "This file isn't a readable jar or zip, so nothing in it was checked.") : null,
    r.partial ? h("p", { class: "banner warn" }, "Partial report: a scan limit was hit, so part of the jar wasn't checked. A partial report is not a clean one.") : null,
    unreadable(r.warnings) ? null : findingsSection(r.findings),
    jarDetails(r),
    warningsSection(r.warnings));
}

// Diff.

const KIND = {
  command: (v) => ["command ", h("code", {}, `/${v}`)],
  permission: (v) => ["permission ", h("code", {}, v)],
  "maven-library": (v) => ["library downloaded at startup ", h("code", {}, v)],
  library: (v) => ["bundled library ", h("code", {}, v)],
};

function itemRow(item, verb) {
  if (item.kind === "class") {
    return h("div", { class: `row sev-${item.severity}` },
      sevBadge(item.severity), h("span", { class: "text" }, `${verb} class `, h("code", {}, item.value)),
      h("code", { class: "rule" }, item.rules.join(", ")));
  }
  return h("div", { class: "row sev-info" }, sevBadge("info"), h("span", { class: "text" }, `${verb} `, KIND[item.kind](item.value)));
}

function removedRow(item) {
  if (item.kind !== "finding") return itemRow(item, "Removed");
  const f = item.finding;
  return h("div", { class: `row sev-${f.severity}` },
    sevBadge(f.severity), h("span", { class: "text" }, f.title, f.value ? [": ", h("code", {}, f.value)] : null), h("code", { class: "rule" }, f.rule));
}

function changeRow(c) {
  const show = (v) => (v == null ? "(none)" : c.kind === "nested" ? v.slice(0, 12) : v);
  const what = {
    library: `Bundled library ${c.key}`,
    "maven-library": `Library downloaded at startup ${c.key}`,
    nested: `Nested jar ${c.key} (SHA-256)`,
  }[c.kind] ?? c.key;
  return h("div", { class: "row" }, h("span", { class: "text" }, what, ": ", h("code", {}, show(c.old)), " → ", h("code", {}, show(c.new))));
}

/** Added items in diff order, with runs of findings grouped like in a report. */
function addedList(items) {
  const out = [];
  let run = [];
  const flush = () => {
    out.push(findingCards(run));
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

function renderDiff(d) {
  const label = (side) => {
    const p = side.plugins[0];
    return p?.name ? `${p.name} ${p.version ?? ""}`.trim() : side.file.name;
  };
  const [a, b] = [label(d.old), label(d.new)];
  const sameName = d.old.plugins[0]?.name && d.old.plugins[0].name === d.new.plugins[0]?.name;
  const title = sameName ? `${a} → ${d.new.plugins[0].version ?? "?"}` : `${a} → ${b}`;
  const addedFindings = d.added.filter((i) => i.kind === "finding").map((i) => i.finding);

  const unchanged = [];
  for (const k of d.unchanged) {
    const last = unchanged.at(-1);
    if (last?.rule === k.rule) last.n++;
    else unchanged.push({ rule: k.rule, n: 1 });
  }

  return h("article", { class: "report" },
    h("header", { class: "report-head" },
      h("h2", { tabindex: "-1" }, title),
      h("p", { class: "meta" }, `${d.old.file.name} → ${d.new.file.name} · obfuscation ${d.obfuscation.old} → ${d.obfuscation.new}`),
      h("div", { class: "head-row" }, countList(counts(addedFindings), " added"),
        downloadButton(`${d.old.file.name}-vs-${d.new.file.name}.jarsnitch.json`, d))),
    d.warnings.map((w) => h("p", { class: "banner warn" }, w.message)),
    h("section", { class: "section" },
      h("h3", {}, "This update adds"),
      d.added.length
        ? h("div", { class: "list" }, addedList(d.added))
        : h("p", {}, "Nothing new from the rule list.")),
    h("section", { class: "section" },
      h("h3", {}, "This update removes"),
      d.removed.length ? h("div", { class: "list" }, d.removed.map(removedRow)) : h("p", { class: "muted" }, "Nothing.")),
    d.changed.length
      ? h("section", { class: "section" }, h("h3", {}, "Changes"), h("div", { class: "list" }, d.changed.map(changeRow)))
      : null,
    unchanged.length
      ? h("section", { class: "section" },
          h("h3", {}, "Unchanged"),
          h("ul", { class: "chips" }, unchanged.map((u) => h("li", {}, h("code", {}, u.rule), u.n > 1 ? ` ×${u.n}` : null))))
      : null);
}

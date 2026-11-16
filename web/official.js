// "Check official releases": compares jars with the files their projects published on
// Modrinth and Hangar, and looks for updates on Modrinth. The page's only network use after
// it loads, and only when the button is pressed. The CLI's `check` does the same
// (crates/cli/src/check.rs); this follows it, minus Hangar downloads.

const MODRINTH = "https://api.modrinth.com/v2";
const HANGAR = "https://hangar.papermc.io/api/v1";
const MAX_BODY = 256e6;
const MAX_CANDIDATES = 3;
const TIMEOUT = 120_000;

const LOADERS = {
  bukkit: ["bukkit", "spigot", "paper", "purpur", "folia"],
  paper: ["paper", "purpur", "folia"],
  bungee: ["bungeecord", "waterfall"],
  velocity: ["velocity"],
  fabric: ["fabric"],
  quilt: ["quilt"],
  forge: ["forge"],
  neoforge: ["neoforge"],
};
const SERVER = new Set(["bukkit", "paper", "bungee", "velocity"]);
const ORDER = ["bukkit", "paper", "velocity", "bungee", "fabric", "quilt", "neoforge", "forge"];

/** The descriptor that names the plugin: plugin.yml before the others. */
export const mainPlugin = (r) => [...r.plugins].sort((a, b) => ORDER.indexOf(a.platform) - ORDER.indexOf(b.platform))[0];

const hex = (buf) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
const digest = async (algo, bytes) => hex(await crypto.subtle.digest(algo, bytes));

const PRE = ["snapshot", "dev", "alpha", "beta", "rc", "pre", "preview", "nightly", "build", "test", "experimental", "unstable", "canary", "ea"];

/**
 * A lowercase version tail with a pre-release word in it: "-snapshot", "-rc1", "+build.5",
 * "-m2", "-b45". Whole words only, so "-premium" and "-sources" aren't.
 */
const prerelease = (tail) =>
  tail.split(/[^a-z0-9]/).some((token) => {
    const word = token.replace(/\d+$/, "");
    return PRE.includes(word) || ((word === "m" || word === "b") && word.length < token.length);
  });

/**
 * "1.4.28" matches "v1.4.28" and "1.4.28-bukkit", not "1.4.2", "1.4.28.1", "1.4.28-SNAPSHOT",
 * "1.4.28-beta.1" or "1.4.28-b45": a development or pre-release build isn't the release.
 */
export function sameVersion(claimed, listed) {
  const norm = (s) => s.trim().replace(/^[vV]+/, "").toLowerCase();
  const [a, b] = [norm(claimed), norm(listed)];
  const extends_ = (long, short) => {
    const tail = long.slice(short.length);
    return long.startsWith(short) && /^[-+_ (]/.test(tail) && !prerelease(tail);
  };
  return a !== "" && (a === b || extends_(b, a) || extends_(a, b));
}

/** A file name without its version, so Bukkit and Paper builds of one release differ. */
export const stem = (file) => file.toLowerCase().replace(/[\d.\-_+]/g, "");

/** The same version name, ignoring case and a leading v. */
export function exactVersion(a, b) {
  const norm = (s) => s.trim().replace(/^[vV]+/, "").toLowerCase();
  return norm(a) !== "" && norm(a) === norm(b);
}

/** A version without its suffix: "5.29" of "5.29-paper", "1.0.0" of "v1.0.0+build.5". */
export const baseVersion = (v) => v.trim().replace(/^[vV]+/, "").toLowerCase().split(/[-+_ (]/)[0];

/** A file name, lowercase, without its release's version: builds for other Minecraft versions stay apart. */
export function bare(file, version) {
  const v = version.replace(/^[vV]+/, "").toLowerCase();
  return v ? file.toLowerCase().replace(v, "") : file.toLowerCase();
}

// A project-name change must preserve explicit platform words, including those in the
// release version, and numeric Minecraft/build markers outside that version.
function renameMarkers(file, version) {
  const platforms = new Set(["bukkit", "spigot", "paper", "purpur", "folia", "fabric", "quilt", "forge", "neoforge", "velocity", "bungee", "bungeecord", "waterfall"]);
  const words = [...new Set(file.toLowerCase().split(/[^a-z0-9]/).filter((s) => platforms.has(s)))].sort();
  const numbers = bare(file, version).split(/[^\d.]/).map((s) => s.replace(/^\.+|\.+$/g, "")).filter(Boolean);
  return JSON.stringify([words, numbers, minecraftMarkers(file)]);
}

// Explicit MC labels must survive removal of the release version itself.
function minecraftMarkers(file) {
  const parts = file.toLowerCase().split(/[^a-z0-9.]/);
  return parts.flatMap((s, i) => {
    const prefix = /^(?:minecraft|mc)(.*)$/.exec(s);
    if (!prefix) return [];
    const tail = prefix[1] || parts[i + 1] || "";
    const number = /^[\d.]*/.exec(tail)[0].replace(/^\.+|\.+$/g, "");
    return number ? [number] : [];
  });
}

/** Whether `published` is after `than`: to the second, then the fraction to 9 digits. An unknown date counts as later. */
export function newer(published, than) {
  if (!published || !than) return true;
  const key = (s) => [s.slice(0, 19), (/^\.(\d{0,9})/.exec(s.slice(19))?.[1] ?? "").padEnd(9, "0")];
  const [p, t] = [key(String(published)), key(String(than))];
  return p[0] > t[0] || (p[0] === t[0] && p[1] > t[1]);
}

let last = 0;
/** Stays under Modrinth's 300 and Hangar's 240 requests a minute. */
async function pace(url) {
  if (!url.startsWith(MODRINTH) && !url.startsWith(HANGAR)) return;
  const wait = last + 260 - performance.now();
  if (wait > 0) await new Promise((r) => setTimeout(r, wait));
  last = performance.now();
}

/** A response body (parsed JSON unless `bytes`), or null when a GET answers 404. `signal` stops it; each try gives up after TIMEOUT. */
async function request(url, { json, bytes = false, signal } = {}) {
  const short = url.split("?")[0];
  const host = new URL(url).host;
  for (let attempt = 0; attempt < 2; attempt++) {
    signal?.throwIfAborted();
    await pace(url);
    const timeout = AbortSignal.timeout(TIMEOUT);
    // ponytail: without AbortSignal.any (older browsers) a new scan stops the next request, not this one.
    const either = signal && AbortSignal.any ? AbortSignal.any([signal, timeout]) : timeout;
    const stopped = () =>
      timeout.aborted ? new Error(`${host} didn't answer within ${TIMEOUT / 60_000} minutes`) : signal?.aborted ? new Error("the check was stopped") : null;
    let res;
    try {
      res = await fetch(url, {
        method: json ? "POST" : "GET",
        headers: json ? { "Content-Type": "application/json" } : undefined,
        body: json ? JSON.stringify(json) : undefined,
        referrerPolicy: "no-referrer",
        credentials: "omit",
        signal: either,
      });
    } catch {
      throw stopped() ?? new Error(`couldn't reach ${host}`);
    }
    if (res.status === 404) {
      if (json) throw new Error(`${short}: HTTP 404`);
      return null;
    }
    if (res.status === 429 && attempt === 0) {
      await new Promise((r) => setTimeout(r, 10_000));
      continue;
    }
    if (!res.ok) throw new Error(`${short}: HTTP ${res.status}`);
    if (Number(res.headers.get("content-length")) > MAX_BODY) throw new Error(`${short}: file too large`);
    let body;
    try {
      body = await read(res, short);
    } catch (e) {
      throw stopped() ?? e;
    }
    if (bytes) return body;
    try {
      return JSON.parse(new TextDecoder().decode(body));
    } catch {
      throw new Error(`${short}: unexpected response`);
    }
  }
  throw new Error(`${short}: rate limited`);
}

/** The body, counted as it arrives: Content-Length can be missing or wrong. */
async function read(res, short) {
  if (!res.body) return new Uint8Array(0);
  const reader = res.body.getReader();
  const chunks = [];
  let size = 0;
  for (let part; !(part = await reader.read()).done; ) {
    size += part.value.length;
    if (size > MAX_BODY) {
      reader.cancel().catch(() => {});
      throw new Error(`${short}: file too large`);
    }
    chunks.push(part.value);
  }
  const body = new Uint8Array(size);
  let at = 0;
  for (const c of chunks) {
    body.set(c, at);
    at += c.length;
  }
  return body;
}

const q = (v) => encodeURIComponent(typeof v === "string" ? v : JSON.stringify(v));

function modrinthSource(project, version, file) {
  return {
    site: "modrinth",
    project: project.title,
    url: `https://modrinth.com/project/${project.slug}`,
    version: version.version_number,
    published: version.date_published || null,
    file: file.filename,
    download: file.url,
    diff: null,
  };
}

const jarFiles = (v) => v.files.filter((f) => /\.jar$/i.test(f.filename));

/** Letters and digits in any script, lowercased: "Chunky Border" and "chunky-border" are both "chunkyborder". */
const squash = (s) => String(s ?? "").toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");

/** The project's slug or title and the plugin's name contain one another. An unknown name passes. */
function namedLike(project, name) {
  const n = squash(name);
  return !n || [project.slug, project.title].some((p) => {
    const s = squash(p);
    return s !== "" && (s.includes(n) || n.includes(s));
  });
}

/**
 * Checks jars, `[{ file, report }]`, against Modrinth and Hangar. `diff(oldFile, newFile)`
 * compares two files and resolves to the diff JSON. `progress(text)` reports each step.
 * `signal` stops the requests still to come. Resolves to one result per jar, shaped like the
 * CLI's `check --json`, plus `hangar` (Hangar was asked about the bytes) and `official.named`
 * (the project is named like the plugin).
 */
export async function checkAll(entries, { diff, progress = () => {}, updates = true, signal }) {
  const jars = [];
  for (const { file, report } of entries) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const plugin = mainPlugin(report);
    const loaders = [...new Set(report.plugins.flatMap((p) => LOADERS[p.platform] ?? []))];
    const server = report.plugins.some((p) => SERVER.has(p.platform));
    jars.push({
      file, report, loaders, server,
      sha512: await digest("SHA-512", bytes),
      home: null,
      out: {
        file: file.name,
        sha256: report.file.sha256,
        plugin: plugin?.name ?? null,
        version: plugin?.version ?? null,
        status: report.warnings.some((w) => w.code === "zip.unreadable" && !w.location) ? "unreadable" : "not-found",
        hangar: server,
        official: null,
        unlike: null,
        update: null,
        errors: [],
      },
    });
  }
  const live = jars.filter((j) => j.out.status !== "unreadable");
  const attempt = async (j, step) => {
    try {
      await step(j);
    } catch (e) {
      j.out.errors.push(e.message);
    }
  };

  progress("Looking up the files on Modrinth…");
  try {
    await byHashModrinth(live, signal);
  } catch (e) {
    for (const j of live) j.out.errors.push(e.message);
  }
  for (const j of live.filter((j) => !j.home && j.server)) {
    progress(`Looking up ${j.file.name} on Hangar…`);
    await attempt(j, (j) => byHashHangar(j, signal));
  }
  // Only jars neither site has by hash: a Hangar match stays official and its name isn't searched for.
  for (const j of live.filter((j) => j.out.status === "not-found")) {
    progress(`Looking for the official ${j.out.plugin ?? j.file.name} ${j.out.version ?? ""}…`);
    await attempt(j, (j) => locate(j, diff, signal));
  }
  if (updates) {
    progress("Looking for updates…");
    try {
      await updatesModrinth(live, diff, progress, signal);
    } catch (e) {
      for (const j of live) j.out.errors.push(e.message);
    }
  }
  return jars.map((j) => j.out);
}

/** A JSON object of hash -> version, or an error, as the command line has. */
function map(found, url) {
  if (!found || typeof found !== "object" || Array.isArray(found)) throw new Error(`${url}: unexpected response`);
  return found;
}

async function projects(ids, signal) {
  return ids.length ? (await request(`${MODRINTH}/projects?ids=${q(ids)}`, { signal })) ?? [] : [];
}

async function byHashModrinth(jars, signal) {
  if (!jars.length) return;
  const found = map(await request(`${MODRINTH}/version_files`, { json: { hashes: jars.map((j) => j.sha512), algorithm: "sha512" }, signal }), `${MODRINTH}/version_files`);
  const list = await projects([...new Set(Object.values(found).map((v) => v.project_id))], signal);
  for (const j of jars) {
    const version = found[j.sha512];
    const project = version && list.find((p) => p.id === version.project_id);
    const file = version?.files.find((f) => f.hashes.sha512 === j.sha512);
    if (!project || !file) continue;
    j.out.status = "official";
    // Anyone can upload a copy: say so when the project that lists these bytes is named otherwise.
    j.out.official = { ...modrinthSource(project, version, file), named: namedLike(project, j.out.plugin) };
    j.home = { project, file };
  }
}

async function byHashHangar(j, signal) {
  const p = await request(`${HANGAR}/versions/hash/${j.out.sha256}`, { signal });
  if (!p?.namespace) return;
  const { owner, slug } = p.namespace;
  j.out.status = "official";
  const project = p.name ?? slug;
  j.out.official = {
    site: "hangar", project, url: `https://hangar.papermc.io/${owner}/${slug}`, version: null, published: null, file: null, download: null, diff: null,
    named: namedLike({ slug, title: project }, j.out.plugin),
  };
}

/**
 * Downloads an official file, checks its hash, and compares it with the jar: `{ diff, same }`.
 * `same` when it's the same plugin: the same name, and the same main class or at least half of
 * the official classes (so a changed `main:` can't hide a modified copy).
 */
async function compareWith(j, file, diff, signal) {
  const bytes = await request(file.url, { bytes: true, signal });
  if (!bytes) throw new Error(`${file.url}: HTTP 404`);
  if ((await digest("SHA-512", bytes)) !== file.hashes.sha512) throw new Error(`${file.url}: the download doesn't match the hash Modrinth lists`);
  const d = await diff(new File([bytes], file.filename), j.file);
  const ours = mainPlugin(j.report);
  const theirs = d.old.plugins.find((p) => p.platform === ours?.platform);
  const c = d.code;
  const shared = c.unchanged_classes + c.changed_classes.length;
  const mostlySame = shared > 0 && shared >= c.removed_classes.length;
  const sameName = theirs?.name && ours?.name && theirs.name.toLowerCase() === ours.name.toLowerCase();
  return { diff: d, same: Boolean(sameName && (theirs.main === ours.main || mostlySame)) };
}

const distance = (d) => {
  const c = d.code;
  return c.changed_classes.length + c.added_classes.length + c.removed_classes.length + c.changed_files.length + c.added_files.length + c.removed_files.length;
};

/** Same code, as the core decides it (`same_code` in the diff JSON), so the page and the CLI agree. */
export const sameCode = (d) => d.same_code === true;

/** The diff adds a High finding, like the core's `Diff::adds_high`. */
export const addsHigh = (d) => d?.added?.some((i) => i.kind === "finding" && i.finding.severity === "high") === true;

/** The CLI's `Checked::alarming`: the code differs from the official file, or this jar or its update adds a High finding. */
export const alarming = (c) => c.status === "differs" || addsHigh(c.official?.diff) || addsHigh(c.update?.diff);

async function locate(j, diff, signal) {
  const { plugin: name, version: claimed } = j.out;
  if (!name || !claimed || !j.loaders.length) return;
  // A failed request or a bad download skips that candidate, not the rest, and is reported: the
  // result may then come from a less likely file. A stopped check stops.
  const errors = [];
  const tried = async (step, fallback) => {
    try {
      return await step();
    } catch (e) {
      if (signal?.aborted) throw e;
      errors.push(e.message);
      return fallback;
    }
  };
  const guess = name.toLowerCase().replaceAll(" ", "-");
  const candidates = await tried(() => projects([guess], signal), []);
  const facets = [[`project_type:${j.server ? "plugin" : "mod"}`]];
  const hits = await tried(() => request(`${MODRINTH}/search?query=${q(name)}&facets=${q(facets)}&limit=3`, { signal }), null);
  const more = (hits?.hits ?? []).map((h) => h.project_id).filter((id) => !candidates.some((c) => c.id === id));
  candidates.push(...(await tried(() => projects(more, signal), [])));
  let best = null;
  // A file of this version in the plugin's own project that isn't the same plugin.
  let unlike = null;
  let downloads = 0;
  projects: for (const project of candidates.slice(0, 3)) {
    const versions = (await tried(() => request(`${MODRINTH}/project/${project.id}/version?loaders=${q(j.loaders)}&include_changelog=false`, { signal }), null)) ?? [];
    const matching = versions.filter((v) => sameVersion(claimed, v.version_number)).sort((a, b) => (b.version_number.toLowerCase() === claimed.toLowerCase()) - (a.version_number.toLowerCase() === claimed.toLowerCase()));
    for (const version of matching) {
      const files = jarFiles(version).sort((a, b) => (b.filename === j.file.name) - (a.filename === j.file.name) || b.primary - a.primary);
      for (const file of files) {
        if (downloads++ >= MAX_CANDIDATES) break projects;
        const r = await tried(() => compareWith(j, file, diff, signal), null);
        if (r?.same) {
          if (!best || distance(r.diff) < distance(best.diff)) best = { diff: r.diff, project, version, file };
          if (distance(best.diff) === 0) break projects;
        } else if (r && !unlike && project.slug === guess) {
          unlike = { ...modrinthSource(project, version, file), diff: r.diff };
        }
      }
    }
    if (best) break;
  }
  if (best) {
    j.out.status = sameCode(best.diff) ? "same-code" : "differs";
    j.out.official = { ...modrinthSource(best.project, best.version, best.file), diff: best.diff, named: namedLike(best.project, name) };
    j.home = { project: best.project, file: best.file };
  } else {
    j.out.unlike = unlike;
  }
  j.out.errors.push(...errors);
}

async function updatesModrinth(jars, diff, progress, signal) {
  const homed = jars.filter((j) => j.home);
  if (!homed.length) return;
  const hashes = homed.map((j) => ({ hash: j.home.file.hashes.sha512, loaders: j.loaders, game_versions: [], version_types: ["release"] }));
  const url = `${MODRINTH}/version_files/update_individual`;
  const found = map(await request(url, { json: { algorithm: "sha512", hashes }, signal }), url);
  for (const j of homed) {
    const newest = found[j.home.file.hashes.sha512];
    if (!newest) continue;
    try {
      const pick = await updateFor(j, newest, signal);
      if (!pick) continue;
      progress(`Comparing ${j.file.name} with ${pick.version.version_number}…`);
      const source = modrinthSource(j.home.project, pick.version, pick.file);
      source.url = `https://modrinth.com/project/${j.home.project.slug}/version/${pick.version.id}`;
      const bytes = await request(pick.file.url, { bytes: true, signal });
      if (!bytes) throw new Error(`${pick.file.url}: HTTP 404`);
      if ((await digest("SHA-512", bytes)) !== pick.file.hashes.sha512) throw new Error(`${pick.file.url}: the download doesn't match the hash Modrinth lists`);
      source.diff = await diff(j.file, new File([bytes], pick.file.filename));
      const unreadable = (side) => side.warnings.some((w) => w.code === "zip.unreadable" && !w.location);
      if (unreadable(source.diff.old) || unreadable(source.diff.new)) throw new Error(`${pick.file.url}: the update comparison requires a readable jar`);
      if (source.diff.old.partial || source.diff.new.partial) {
        j.out.errors.push(`${pick.file.filename}: update comparison is incomplete; some contents could not be scanned`);
      }
      j.out.update = source;
    } catch (e) {
      j.out.errors.push(e.message);
    }
  }
}

/** The latest release with a file of the installed build (Bukkit or Paper, Minecraft version), published after the installed one. */
async function updateFor(j, newest, signal) {
  const current = j.home.file;
  const hasCurrent = (v) => v.files.some((f) => f.hashes.sha512 === current.hashes.sha512);
  // The newest release can be older than the installed one (a beta, say). Unknown installed date: no check.
  const later = (v) => newer(v.date_published, j.out.official?.published);
  if (hasCurrent(newest) || !later(newest)) return null;
  const installed = j.out.official?.version ?? "";
  // A release of the installed version itself (the same file again, or another build of it) isn't
  // an update; a later build with a suffix, like 1.0.0-1, can be.
  const own = (v) => exactVersion(installed, v.version_number);
  // Releases of the installed version, suffixes included: 5.29-spigot is a twin of 5.29-paper.
  const twin = (v) => installed !== "" && baseVersion(v.version_number) === baseVersion(installed);
  const flavor = stem(current.filename);
  const mine = bare(current.filename, installed);
  const minecraft = JSON.stringify(minecraftMarkers(current.filename));
  const literal = (file, version) => bare(file, version) !== file.toLowerCase();
  const mineLiteral = literal(current.filename, installed);
  // The installed build's file: the same name stem, and the same name once each release's version
  // is taken out. When a name doesn't hold its release's version, the only file with the same stem.
  // Otherwise none, rather than the wrong build.
  const pick = (v) => {
    const alike = jarFiles(v).filter((f) => stem(f.filename) === flavor && JSON.stringify(minecraftMarkers(f.filename)) === minecraft);
    const exact = alike.find((f) => bare(f.filename, v.version_number) === mine);
    if (exact) return exact;
    const loose = alike.filter((f) => !(mineLiteral && literal(f.filename, v.version_number)));
    return loose.length === 1 ? loose[0] : undefined;
  };
  if (!own(newest) && pick(newest)) return { version: newest, file: pick(newest) };
  // The file names, without versions, of the releases from the installed one back and of the
  // installed version's twins, to tell a project that renamed its files from one that has always
  // had another flavor.
  let older = null;
  const versions = await request(`${MODRINTH}/project/${j.home.project.id}/version?loaders=${q(j.loaders)}&include_changelog=false`, { signal });
  if (versions) {
    older = new Set();
    let back = false;
    for (const v of versions.filter((v) => v.version_type === "release")) {
      // Back at the installed release: nothing newer of this build.
      back ||= hasCurrent(v) || !later(v);
      if (back || twin(v)) jarFiles(v).forEach((f) => older.add(stem(f.filename)));
      if (!back && !own(v) && pick(v)) return { version: v, file: pick(v) };
    }
  }
  // A project that renamed its files: the newest release's only jar, unless it's another flavor
  // of the installed version or a flavor older releases have too.
  const only = jarFiles(newest);
  const renamed = only.length === 1 && !twin(newest) && !older?.has(stem(only[0].filename))
    && renameMarkers(current.filename, installed) === renameMarkers(only[0].filename, newest.version_number);
  return renamed ? { version: newest, file: only[0] } : null;
}

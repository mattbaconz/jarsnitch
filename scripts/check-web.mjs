// Smoke test for web/jarsnitch.wasm through web/engine.js, the code the page runs.
// Build first with scripts/build-web.ps1 or build-web.sh.
//
//   node scripts/check-web.mjs [extra.jar ...]   (extra jars are scanned and timed)

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { Engine } from "../web/engine.js";
import { checkOfficial } from "./check-official.mjs";
import { bare, baseVersion, exactVersion, newer, sameVersion, stem } from "../web/official.js";

const root = new URL("..", import.meta.url);
const read = (path) => readFile(new URL(path, root));
const engine = new Engine(await WebAssembly.compile(await read("web/jarsnitch.wasm")));

// The check's version, flavor and date rules, the same as the command line's.
assert.ok(sameVersion("1.4.28", "v1.4.28-bukkit") && !sameVersion("1.4.28", "1.4.28-SNAPSHOT"));
assert.notEqual(stem("Chunky-Bukkit-1.4.28.jar"), stem("Chunky-Paper-1.4.36.jar"));
assert.equal(bare("Fixture-1.0.0-1.8.jar", "1.0.0"), bare("Fixture-1.1.0-1.8.jar", "v1.1.0"));
assert.notEqual(bare("Fixture-1.0.0-1.8.jar", "1.0.0"), bare("Fixture-1.1.0-1.21.jar", "1.1.0"));
assert.ok(exactVersion("1.0.0", "V1.0.0") && !exactVersion("1.0.0", "1.0.0-1") && !exactVersion("", ""));
assert.deepEqual([baseVersion("5.29-paper"), baseVersion("v1.0.0+build.5")], ["5.29", "1.0.0"]);
assert.ok(newer("2026-10-01T00:00:00.900000Z", "2026-10-01T00:00:00.100000Z") && newer("2026-10-01T00:00:00.1Z", "2026-10-01T00:00:00Z"));
assert.ok(!newer("2026-10-01T00:00:00Z", "2026-10-01T00:00:00.000Z") && !newer("2026-09-30T23:59:59.999Z", "2026-10-01T00:00:00Z"));
assert.ok(newer(undefined, "2026-10-01T00:00:00Z") && newer("", "x"));

const benign = engine.scan("benign-plugin.jar", await read("fixtures/jars/benign-plugin.jar"));
assert.equal(benign.schema_version, 1);
assert.equal(benign.plugins[0].name, "HelloFixture");
assert.ok(!benign.findings.some((f) => f.severity === "high"), "benign fixture has a High finding");

const d = engine.diff(
  "diff-v1.jar", await read("fixtures/jars/diff-v1.jar"),
  "diff-v2.jar", await read("fixtures/jars/diff-v2.jar"),
);
assert.deepEqual(d.added.map((i) => i.finding.rule), ["code.load", "str.discord-webhook"]);
assert.equal(d.removed.length, 0);

// The page's "Try a sample" jars, copied into web/samples by build-web.
const sample = engine.diff(
  "SampleShop-1.4.0.jar", await read("web/samples/SampleShop-1.4.0.jar"),
  "SampleShop-1.4.1.jar", await read("web/samples/SampleShop-1.4.1.jar"),
);
assert.ok(sample.added.some((i) => i.finding?.rule === "corr.download-and-run"), "the sample update lost its High finding");

const junk = engine.scan("junk.jar", new TextEncoder().encode("not a zip"));
assert.ok(junk.warnings.some((w) => w.code === "zip.unreadable"));

// The same engine keeps working across calls.
assert.equal(engine.scan("again.jar", await read("fixtures/jars/benign-plugin.jar")).file.sha256, benign.file.sha256);

for (const path of process.argv.slice(2)) {
  const bytes = await readFile(path);
  const start = performance.now();
  const r = engine.scan(path, bytes);
  const ms = Math.round(performance.now() - start);
  console.log(`${path}: ${(bytes.length / 1e6).toFixed(1)} MB, ${r.stats.classes} classes, ${r.findings.length} findings, ${ms} ms`);
}
await checkOfficial(engine);
console.log("web engine ok");

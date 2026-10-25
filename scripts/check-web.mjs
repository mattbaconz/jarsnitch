// Smoke test for web/jarsnitch.wasm through web/engine.js, the code the page runs.
// Build first with scripts/build-web.ps1 or build-web.sh.
//
//   node scripts/check-web.mjs [extra.jar ...]   (extra jars are scanned and timed)

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { Engine } from "../web/engine.js";

const root = new URL("..", import.meta.url);
const read = (path) => readFile(new URL(path, root));
const engine = new Engine(await WebAssembly.compile(await read("web/jarsnitch.wasm")));

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
console.log("web engine ok");

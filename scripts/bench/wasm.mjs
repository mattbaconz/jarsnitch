// Times web/jarsnitch.wasm, the engine the web page runs, on every jar in a folder.
// Node runs it on V8, the same engine as Chrome. Build first with scripts/build-web.
//
//   node scripts/bench/wasm.mjs DIR OUT.json

import { readdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Engine } from "../../web/engine.js";

const [dir, out] = process.argv.slice(2);
const wasm = await readFile(new URL("../../web/jarsnitch.wasm", import.meta.url));
const engine = new Engine(await WebAssembly.compile(wasm));
const rows = [];
for (const name of (await readdir(dir)).filter((n) => n.endsWith(".jar")).sort()) {
  const bytes = await readFile(join(dir, name));
  const times = [];
  for (let i = 0; i < 3; i++) {
    const t = performance.now();
    engine.scan(name, bytes);
    times.push(performance.now() - t);
  }
  rows.push({ name, size: bytes.length, ms: times.sort((a, b) => a - b)[1] });
}
const total = rows.reduce((s, r) => s + r.ms, 0);
await writeFile(out, JSON.stringify({ wasm_bytes: wasm.length, node: process.version, total_ms: total, jars: rows }, null, 1));
console.log(`${rows.length} jars in ${(total / 1000).toFixed(2)} s`);
